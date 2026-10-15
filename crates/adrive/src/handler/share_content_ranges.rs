//! Concurrent credential-free ranges into one unpublished pinned file.
use super::{
    download_ranges::{run_ranges, DownloadRange, RangePlan},
    share_content::{failed, validate_etag},
    share_download_progress::RangeProgress,
    share_local::{SafeDir, StagedFile},
    share_manifest::RemoteFile,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    time::Duration,
};
use tos_core::agent::error::CliError;

pub(super) struct RangeDownload<'a> {
    pub address: &'a url::Url,
    pub remote: &'a RemoteFile,
    pub directory: &'a SafeDir,
    pub local: &'a str,
    pub concurrency: usize,
    pub progress: &'a RangeProgress,
}

/// Download verified ranges into an exclusive staged file; errors/cancellation remove unpublished bytes.
pub(super) async fn download(
    client: &reqwest::Client,
    request: RangeDownload<'_>,
) -> Result<(StagedFile, String), CliError> {
    let plan = RangePlan::new(request.remote.size)?;
    let mut staged = StagedFile::new(request.directory, request.local)?;
    staged.file.set_len(request.remote.size)?;
    run_ranges(plan, request.concurrency, |range| {
        let file = &staged.file;
        let request = &request;
        async move {
            transfer_range(client, request, file, range).await?;
            request.progress.completed();
            Ok(())
        }
    })
    .await?;
    let digest = verify_file(&mut staged.file, request.remote).await?;
    Ok((staged, digest))
}

async fn transfer_range(
    client: &reqwest::Client,
    request: &RangeDownload<'_>,
    file: &File,
    range: DownloadRange,
) -> Result<(), CliError> {
    for attempt in 0..3 {
        match attempt_range(client, request, file, range).await {
            Ok(()) => return Ok(()),
            Err((_, true)) if attempt < 2 => {
                tokio::time::sleep(Duration::from_millis(200 << attempt)).await
            }
            Err((error, _)) => return Err(error),
        }
    }
    Err(failed("range retry budget exhausted"))
}

async fn attempt_range(
    client: &reqwest::Client,
    request: &RangeDownload<'_>,
    file: &File,
    range: DownloadRange,
) -> Result<(), (CliError, bool)> {
    let mut builder = client
        .get(request.address.clone())
        .header(reqwest::header::RANGE, range.header())
        .header(reqwest::header::ACCEPT_ENCODING, "identity");
    if !request.remote.etag.is_empty() && !request.remote.etag.starts_with("W/") {
        builder = builder.header(
            reqwest::header::IF_MATCH,
            format!("\"{}\"", request.remote.etag.trim_matches('"')),
        );
    }
    let response = builder
        .send()
        .await
        .map_err(|_| (failed("range connection failed"), true))?;
    let status = response.status();
    if matches!(status.as_u16(), 408 | 429) || status.is_server_error() {
        return Err((
            failed(&format!("range request returned HTTP {}", status.as_u16())),
            true,
        ));
    }
    range
        .validate(
            status.as_u16(),
            response
                .headers()
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|value| value.to_str().ok()),
            request.remote.size,
        )
        .map_err(|error| (error, false))?;
    validate_etag(&response, request.remote).map_err(|error| (error, false))?;
    if response
        .content_length()
        .is_some_and(|size| size != range.size)
    {
        return Err((
            failed("range content length differs from planned range"),
            false,
        ));
    }
    receive_range(response, file, range).await
}

async fn receive_range(
    mut response: reqwest::Response,
    file: &File,
    range: DownloadRange,
) -> Result<(), (CliError, bool)> {
    let mut written = 0_u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| (failed("range stream interrupted"), true))?
    {
        if chunk.len() as u64 > range.size - written {
            return Err((failed("range exceeds planned length"), false));
        }
        write_at(file, &chunk, range.offset + written)
            .map_err(|error| (CliError::Io(error), false))?;
        written += chunk.len() as u64;
    }
    if written != range.size {
        return Err((failed("range body is incomplete"), true));
    }
    Ok(())
}

fn write_at(file: &File, mut bytes: &[u8], mut offset: u64) -> std::io::Result<()> {
    while !bytes.is_empty() {
        #[cfg(unix)]
        let written = std::os::unix::fs::FileExt::write_at(file, bytes, offset)?;
        #[cfg(windows)]
        let written = std::os::windows::fs::FileExt::seek_write(file, bytes, offset)?;
        if written == 0 {
            return Err(std::io::ErrorKind::WriteZero.into());
        }
        bytes = &bytes[written..];
        offset += written as u64;
    }
    Ok(())
}

async fn verify_file(file: &mut File, remote: &RemoteFile) -> Result<String, CliError> {
    file.seek(SeekFrom::Start(0))?;
    let mut sha256 = Sha256::new();
    let mut crc64 = crc64fast::Digest::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut length = 0_u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        length += count as u64;
        sha256.update(&buffer[..count]);
        crc64.write(&buffer[..count]);
        tokio::task::yield_now().await;
    }
    if length != remote.size || (remote.crc64 != 0 && crc64.sum64() != remote.crc64) {
        return Err(failed("content size or CRC64 integrity check failed"));
    }
    Ok(hex::encode(sha256.finalize()))
}
