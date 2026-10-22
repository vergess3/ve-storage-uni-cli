//! Credential-free streamed transfer; only content requests reuse an issued URL.
use super::{
    share_local::{SafeDir, StagedFile},
    share_manifest::RemoteFile,
};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    time::Duration,
};
use tos_core::agent::error::CliError;

/// Validate an issued HTTP(S) URL without echoing credentials or signed queries on failure.

pub(crate) fn validate_url(address: &str) -> Result<url::Url, CliError> {
    let parsed = url::Url::parse(address).map_err(|_| failed("invalid download address"))?;
    // [Review Fix #ShareProtocol2] Accept the service-issued HTTP(S) scheme without imposing an undocumented HTTPS requirement.
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(failed(
            "download address must use HTTP(S) without credentials or fragment",
        ));
    }
    Ok(parsed)
}
pub(super) fn failed(message: &str) -> CliError {
    CliError::TransferFailed(message.into())
}
pub(crate) struct ContentClient {
    client: reqwest::Client,
    runtime: ContentRuntime,
    display: super::share_download_progress::RangeDisplay,
}
#[derive(Clone, Copy)]
pub(super) struct ContentRuntime {
    pub threshold: u64,
    pub concurrency: usize,
}
impl Default for ContentRuntime {
    fn default() -> Self {
        Self {
            threshold: super::high_level::parse_size_bytes(
                tos_core::infra::config::DEFAULT_TRANSFER_CHECKPOINT_THRESHOLD,
            )
            .expect("builtin checkpoint threshold is a valid size"),
            concurrency: tos_core::infra::config::DEFAULT_MULTIPART_CONCURRENCY,
        }
    }
}
impl ContentRuntime {
    /// Resolve cp-compatible explicit transfer controls without reading anonymous account configuration.
    pub(super) fn for_args(args: &crate::cli::share::ShareDownloadArgs) -> Result<Self, CliError> {
        let threshold = args
            .checkpoint_threshold
            .as_deref()
            .map(super::high_level::parse_size_bytes)
            .unwrap_or(Some(Self::default().threshold))
            .filter(|size| *size > 0)
            .ok_or_else(|| {
                CliError::ValidationError(
                    "checkpoint-threshold must be a positive size, for example 20MB".into(),
                )
            })?;
        let concurrency = args
            .multipart_concurrency
            .unwrap_or(Self::default().concurrency);
        if concurrency == 0 {
            return Err(CliError::ValidationError(
                "multipart-concurrency must be positive".into(),
            ));
        }
        Ok(Self {
            threshold,
            concurrency,
        })
    }
}
impl ContentClient {
    /// Build an independent credential-free content transport with bounded idle/connect waits; return initialization errors.
    pub(crate) fn new() -> Result<Self, CliError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| failed("cannot initialize content transport"))?;
        Ok(Self {
            client,
            runtime: ContentRuntime::default(),
            display: Default::default(),
        })
    }
    /// Apply validated transfer controls and stderr-only progress for this download operation.
    pub(super) fn configured(
        mut self,
        runtime: ContentRuntime,
        display: super::share_download_progress::RangeDisplay,
    ) -> Self {
        self.runtime = runtime;
        self.display = display;
        self
    }
    /// Stream this one issued address into a temporary file and return its SHA-256; retry only recoverable content failures.
    pub(crate) async fn download(
        &self,
        address: &str,
        remote: &RemoteFile,
        directory: &SafeDir,
        local: &str,
    ) -> Result<(StagedFile, String), CliError> {
        let parsed = validate_url(address)?;
        // [Review Fix #ShareRange1] Separate Range requests need a pinned version or final expected checksum; otherwise use one representation from a single GET.
        if remote.size >= self.runtime.threshold && remote.has_identity() {
            let progress = self.display.begin(
                remote
                    .size
                    .div_ceil(super::download_ranges::DOWNLOAD_PART_SIZE),
            );
            return super::share_content_ranges::download(
                &self.client,
                super::share_content_ranges::RangeDownload {
                    address: &parsed,
                    remote,
                    directory,
                    local,
                    concurrency: self.runtime.concurrency,
                    progress: &progress,
                },
            )
            .await;
        }
        self.download_single(&parsed, remote, directory, local)
            .await
    }
    async fn download_single(
        &self,
        address: &url::Url,
        remote: &RemoteFile,
        directory: &SafeDir,
        local: &str,
    ) -> Result<(StagedFile, String), CliError> {
        for attempt in 0..3 {
            let result = self.attempt(address, remote, directory, local).await;
            match result {
                Ok(result) => return Ok(result),
                Err((error, true)) if attempt < 2 => {
                    let _ = error;
                    tokio::time::sleep(Duration::from_millis(200 << attempt)).await;
                }
                Err((error, _)) => return Err(error),
            }
        }
        Err(failed("content retry budget exhausted"))
    }
    async fn attempt(
        &self,
        address: &url::Url,
        remote: &RemoteFile,
        directory: &SafeDir,
        local: &str,
    ) -> Result<(StagedFile, String), (CliError, bool)> {
        let mut request = self.client.get(address.clone());
        if !remote.etag.is_empty() && !remote.etag.starts_with("W/") {
            request = request.header(
                reqwest::header::IF_MATCH,
                format!("\"{}\"", remote.etag.trim_matches('"')),
            );
        }
        let response = request
            .send()
            .await
            .map_err(|_| (failed("content connection failed"), true))?;
        let status = response.status();
        if !status.is_success() {
            return Err((
                failed(&format!(
                    "content request returned HTTP {}",
                    status.as_u16()
                )),
                status.is_server_error() || status.as_u16() == 429,
            ));
        }
        validate_response(&response, remote).map_err(|error| (error, false))?;
        let mut staged =
            StagedFile::new(directory, local).map_err(|error| (CliError::Io(error), false))?;
        let digest = receive(response, &mut staged.file, remote).await?;
        Ok((staged, digest))
    }
}
fn validate_response(response: &reqwest::Response, remote: &RemoteFile) -> Result<(), CliError> {
    if response
        .content_length()
        .is_some_and(|length| length != remote.size)
    {
        return Err(failed("content length differs from manifest"));
    }
    validate_etag(response, remote)
}
pub(super) fn validate_etag(
    response: &reqwest::Response,
    remote: &RemoteFile,
) -> Result<(), CliError> {
    if !remote.etag.is_empty() && !remote.etag.starts_with("W/") {
        let received = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok());
        // [Review Fix #Download3] A server ignoring If-Match must not silently bypass the snapshot ETag.
        if received.is_none_or(|etag| etag.trim_matches('"') != remote.etag.trim_matches('"')) {
            return Err(failed("content ETag differs from manifest"));
        }
    }
    Ok(())
}
async fn receive(
    mut response: reqwest::Response,
    file: &mut std::fs::File,
    remote: &RemoteFile,
) -> Result<String, (CliError, bool)> {
    let mut length = 0_u64;
    let mut sha256 = Sha256::new();
    let mut crc64 = crc64fast::Digest::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| (failed("content stream interrupted"), true))?
    {
        length = length
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| (failed("content length overflow"), false))?;
        if length > remote.size {
            return Err((failed("content exceeds manifest size"), false));
        }
        file.write_all(&chunk)
            .map_err(|error| (CliError::Io(error), false))?;
        sha256.update(&chunk);
        crc64.write(&chunk);
    }
    if length != remote.size || (remote.crc64 != 0 && crc64.sum64() != remote.crc64) {
        return Err((
            failed("content size or CRC64 integrity check failed"),
            false,
        ));
    }
    Ok(hex::encode(sha256.finalize()))
}
/// Hash a native no-follow ordinary file within the expected size bound; yield for cancellation and report read/size errors.
pub(crate) async fn local_digest(
    directory: &SafeDir,
    local: &str,
    expected_size: u64,
) -> Result<(u64, String), CliError> {
    let (parent, name) = directory.parent(local, false)?;
    let mut file = parent.read(&name)?;
    let mut length = 0_u64;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        length = length
            .checked_add(count as u64)
            .ok_or_else(|| failed("local size overflow"))?;
        if length > expected_size {
            // [Review Fix #D03] A completed checkpoint no longer matches local bytes; retrying transfer cannot resolve this conflict.
            return Err(CliError::Conflict(
                "completed local file changed since checkpoint".into(),
            ));
        }
        digest.update(&buffer[..count]);
        tokio::task::yield_now().await;
    }
    Ok((length, hex::encode(digest.finalize())))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_http_and_https_but_rejects_credentials_and_non_http_urls() {
        for address in [
            "https://user:pass@example.test/file",
            "http://user:pass@example.test/file",
            "http://example.test/file#secret",
            "file:///etc/passwd",
            "ftp://example.test",
        ] {
            let error = validate_url(address).unwrap_err();
            assert!(!error.to_string().contains("secret"));
            assert!(!error.to_string().contains("pass"));
        }
        assert!(validate_url("http://example.test/file?secret=token").is_ok());
        assert!(validate_url("https://example.test/file?secret=token").is_ok());
        assert!(validate_url("http://127.0.0.1:1234/a").is_ok());
    }
}

#[cfg(test)]
mod integrity_tests {
    use super::*;
    use crate::domain::share_access::fixture::{reply, Fixture};
    #[tokio::test]
    async fn rejects_missing_changed_etag_and_crc_before_publication() {
        for (response, crc) in [
            (reply(200, "abc"), 0),
            (
                reply(200, "abc").replace("Connection: close", "ETag: xyz\r\nConnection: close"),
                0,
            ),
            (
                reply(200, "abc").replace("Connection: close", "ETag: abc\r\nConnection: close"),
                123,
            ),
        ] {
            let fixture = Fixture::new(vec![response]);
            let root = std::env::temp_dir()
                .canonicalize()
                .unwrap()
                .join(format!("share-integrity-{}", ulid::Ulid::new()));
            let directory = SafeDir::open(&root, true).unwrap();
            let remote = RemoteFile {
                path: "a".into(),
                size: 3,
                etag: "abc".into(),
                crc64: crc,
                updated_at: 0,
            };
            assert!(ContentClient::new()
                .unwrap()
                .download(
                    &format!("{}/signed?secret=hidden", fixture.endpoint),
                    &remote,
                    &directory,
                    "a"
                )
                .await
                .is_err());
            assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
            std::fs::remove_dir_all(root).unwrap();
            fixture.finish();
        }
    }
}
