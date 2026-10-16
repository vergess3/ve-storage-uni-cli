//! Aggregate download orchestration, cancellation, and file-level resume.
use super::{
    common::{map_ids_error, output_result},
    share_checkpoint::{Checkpoint, Identity, Journal},
    share_content::{local_digest, ContentClient},
    share_download_progress::DownloadProgress,
    share_local::{resolve_directory, SafeDir},
    share_manifest::{enumerate, Entry, RemoteFile},
    share_receive::ShareReceiver,
};
use crate::cli::share::ShareDownloadArgs;
use crate::domain::client::Error;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tos_core::agent::{
    envelope::{Envelope, ErrorDetail, ErrorKind, Status},
    error::{CliError, ExitCode},
    global_args::GlobalArgs,
    request_id::sanitize_request_id,
};
use unicode_normalization::UnicodeNormalization;

#[derive(Default)]
struct Progress {
    journal: Option<Journal>,
    checkpoint: Option<Checkpoint>,
    is_checkpoint_durable: bool,
    active: Option<usize>,
    is_issuing: bool,
    failure_status_code: Option<u16>,
    failure_request_id: Option<String>,
    skipped: usize,
    runtime: super::share_content::ContentRuntime,
    is_resuming: bool,
    verified: Vec<bool>,
    display: DownloadProgress,
}
/// Execute a receiver download and print exactly one aggregate envelope, including failures.
/// Returns the aggregate exit code, or an output serialization/write error.
pub(crate) async fn download(
    global: &GlobalArgs,
    auth: &crate::cli::ADriveAuthArgs,
    args: &ShareDownloadArgs,
) -> Result<i32, CliError> {
    let mut progress = Progress {
        display: DownloadProgress::new(global, args),
        ..Progress::default()
    };
    let result = tokio::select! {
        biased;
        signal = tokio::signal::ctrl_c() => {
            match signal {
                Ok(()) => Err(CliError::TransferFailed("download interrupted; completed files retained".into())),
                Err(error) => Err(CliError::Io(error)),
            }
        }
        result = execute_with_auth(global,auth,args,&mut progress) => result,
    };
    progress.display.finish();
    let guidance = result
        .as_ref()
        .err()
        .filter(|error| {
            auth.auth_mode == Some(crate::domain::auth::AuthMode::Oauth)
                && progress.journal.is_none()
                && matches!(error, CliError::AuthFailed(_) | CliError::ConfigMissing(_))
        })
        .map(|error| {
            super::common::adrive_error_guidance(
                global,
                auth.auth_mode,
                error,
                "ve-adrive share download",
            )
        });
    let (mut envelope, code) = aggregate(&progress, result.err());
    // [Review Fix #ShareReceiveGuidance1] Aggregate failures must expose the same explicit OAuth login workflow as inspect/files.
    if let (Some(error), Some(guidance)) = (&mut envelope.error, guidance) {
        error.fix_command = guidance.fix_command;
        error.suggested_action = Some(guidance.suggested_action);
        error.doctor_hint = guidance.doctor_hint;
    }
    output_result(global, &envelope)?;
    Ok(code)
}
async fn execute_with_auth(
    global: &GlobalArgs,
    auth: &crate::cli::ADriveAuthArgs,
    args: &ShareDownloadArgs,
    progress: &mut Progress,
) -> Result<(), CliError> {
    progress.is_resuming = args.is_resume;
    progress.runtime = super::share_content::ContentRuntime::for_args(args)?;
    let mut receiver = ShareReceiver::for_command(global, auth, &args.share_url)?;
    let info = receiver.info().await?;
    receiver
        .authorize(global, &info, args.is_password_stdin)
        .await?;
    // [Review Fix #LiveRoot2] Persist actual root identity so aliases share resume/collision rules; pin it before remote writes.
    let destination = resolve_directory(Path::new(&args.destination_dir), true)?;
    let directory = SafeDir::open_resolved(&destination)?;
    let identity = task_identity(&receiver, args, &destination);
    if let Some(path) = &args.checkpoint {
        progress.checkpoint = Some(Checkpoint::open(Path::new(path), args.is_resume)?);
    }
    progress.display.begin_scan();
    let journal = load_or_enumerate(
        &mut receiver,
        args,
        progress,
        identity,
        info.file_path_list.as_deref(),
    )
    .await?;
    journal.validate(&journal.identity)?;
    progress.verified = vec![!args.is_resume; journal.manifest.files.len()];
    // [Review Fix #Download1] Preserve a completed plan even when local preflight rejects it.
    progress.journal = Some(journal);
    let journal = progress
        .journal
        .as_ref()
        .ok_or_else(|| invalid("missing manifest"))?;
    check_reserved(journal, progress.checkpoint.as_ref().map(Checkpoint::path))?;
    preflight(&directory, journal, args.is_resume).await?;
    if args.is_resume {
        validate_resume(&mut receiver, &directory, progress).await?;
    }
    save_progress(progress)?;
    begin_transfer_display(progress);
    transfer_files(&mut receiver, &directory, progress).await
}

fn begin_transfer_display(progress: &mut Progress) {
    if let Some(journal) = &progress.journal {
        progress.skipped = journal
            .completed
            .iter()
            .filter(|digest| digest.is_some())
            .count();
        progress
            .display
            .begin_transfer(journal.manifest.files.len(), progress.skipped);
    }
}
#[cfg(test)]
async fn execute(
    global: &GlobalArgs,
    args: &ShareDownloadArgs,
    progress: &mut Progress,
) -> Result<(), CliError> {
    execute_with_auth(
        global,
        &crate::cli::ADriveAuthArgs::default(),
        args,
        progress,
    )
    .await
}
async fn load_or_enumerate(
    receiver: &mut ShareReceiver,
    args: &ShareDownloadArgs,
    progress: &Progress,
    identity: Identity,
    file_paths: Option<&[String]>,
) -> Result<Journal, CliError> {
    if args.is_resume {
        progress
            .checkpoint
            .as_ref()
            .ok_or_else(|| invalid("--resume requires --checkpoint"))?
            .load(&identity)
    } else {
        let manifest = enumerate(
            receiver,
            args.path.as_deref(),
            args.is_recursive,
            file_paths,
        )
        .await?;
        Ok(Journal {
            version: 1,
            identity,
            completed: vec![None; manifest.files.len()],
            manifest,
        })
    }
}
pub(super) fn task_identity(
    receiver: &ShareReceiver,
    args: &ShareDownloadArgs,
    destination: &Path,
) -> Identity {
    Identity {
        origin: receiver.identity.origin.clone(),
        share_id: receiver.identity.share_id.clone(),
        selection: args.path.clone(),
        destination: destination.to_string_lossy().into(),
        is_recursive: args.is_recursive,
        is_overwrite: args.is_overwrite,
        api_origin: Some(receiver.client.api_origin()),
        recipient: receiver.auth_context.clone(),
    }
}
fn invalid(message: &str) -> CliError {
    CliError::ValidationError(message.into())
}
pub(super) fn check_reserved(journal: &Journal, checkpoint: Option<&Path>) -> Result<(), CliError> {
    let destination = Path::new(&journal.identity.destination);
    for relative in journal
        .manifest
        .directories
        .iter()
        .chain(journal.manifest.files.iter().map(|entry| &entry.local))
    {
        if relative
            .split('/')
            .any(|part| part.starts_with(".adrive-share-"))
        {
            return Err(invalid("manifest uses a reserved temporary filename"));
        }
        if let Some(checkpoint) = checkpoint {
            let target = folded(&destination.join(relative));
            for protected in [
                checkpoint.to_owned(),
                PathBuf::from(format!("{}.lock", checkpoint.display())),
            ] {
                let protected = folded(&protected);
                if target == protected
                    || protected.starts_with(&format!("{target}/"))
                    || target.starts_with(&format!("{protected}/"))
                {
                    return Err(invalid("checkpoint/lock collides with download mapping"));
                }
            }
        }
    }
    Ok(())
}
fn folded(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .nfd()
        .flat_map(char::to_uppercase)
        .collect()
}
async fn preflight(
    directory: &SafeDir,
    journal: &Journal,
    is_resume: bool,
) -> Result<(), CliError> {
    for relative in &journal.manifest.directories {
        directory.directory(relative, true)?;
        tokio::task::yield_now().await;
    }
    for (index, entry) in journal.manifest.files.iter().enumerate() {
        tokio::task::yield_now().await;
        let (parent, name) = directory.parent(&entry.local, true)?;
        if parent.ordinary(&name)?
            && !journal.identity.is_overwrite
            && !(is_resume && journal.completed[index].is_some())
        {
            return Err(CliError::Conflict(
                "destination file exists; use --overwrite or the matching completed checkpoint"
                    .into(),
            ));
        }
    }
    Ok(())
}
async fn validate_resume(
    receiver: &mut ShareReceiver,
    directory: &SafeDir,
    progress: &mut Progress,
) -> Result<(), CliError> {
    let journal = progress
        .journal
        .as_ref()
        .ok_or_else(|| invalid("missing manifest"))?;
    for (index, entry) in journal.manifest.files.iter().enumerate() {
        progress.active = Some(index);
        if verify_resume_entry(
            receiver,
            Some(directory),
            entry,
            journal.completed[index].as_deref(),
        )
        .await?
        {
            progress.skipped += 1;
        }
        progress.verified[index] = true;
        progress.active = None;
    }
    Ok(())
}

/// Verify remote identity and any completed local digest without modifying a checkpoint.
/// `directory` may be absent for pending entries; `expected` is the completed SHA256, if any.
/// Returns whether the entry is completed; mismatched/missing completed bytes are conflicts, while native I/O errors propagate.
pub(super) async fn verify_resume_entry(
    receiver: &mut ShareReceiver,
    directory: Option<&SafeDir>,
    entry: &Entry,
    expected: Option<&str>,
) -> Result<bool, CliError> {
    if !entry.remote.has_identity() {
        return Err(invalid(
            "resume requires a strong ETag or CRC64 remote identity",
        ));
    }
    let current = receiver
        .ensure_session()
        .await?
        .get_file(&entry.remote.path)
        .await
        .map_err(map_ids_error)?;
    // [Review Fix #D03] Checkpoint identity/content changes use the cp-compatible conflict category.
    if RemoteFile::from_info(current)? != entry.remote {
        return Err(CliError::Conflict(
            "remote file changed since checkpoint".into(),
        ));
    }
    let Some(expected) = expected else {
        return Ok(false);
    };
    let directory = directory.ok_or_else(|| {
        CliError::Conflict("completed local file missing since checkpoint".into())
    })?;
    let (size, digest) = local_digest(directory, &entry.local, entry.remote.size)
        .await
        .map_err(|error| match error {
            CliError::Io(error) if error.kind() == std::io::ErrorKind::NotFound => {
                CliError::Conflict("completed local file missing since checkpoint".into())
            }
            error => error,
        })?;
    if size != entry.remote.size || digest != expected {
        return Err(CliError::Conflict(
            "completed local file changed since checkpoint".into(),
        ));
    }
    Ok(true)
}
fn save_progress(progress: &mut Progress) -> Result<(), CliError> {
    if let (Some(checkpoint), Some(journal)) = (&progress.checkpoint, &progress.journal) {
        // [Review Fix #Download2] A failed save must not claim the latest published set is recoverable.
        progress.is_checkpoint_durable = false;
        checkpoint.save(journal)?;
        progress.is_checkpoint_durable = true;
    }
    Ok(())
}
async fn transfer_files(
    receiver: &mut ShareReceiver,
    directory: &SafeDir,
    progress: &mut Progress,
) -> Result<(), CliError> {
    let content =
        ContentClient::new()?.configured(progress.runtime, progress.display.range_display());
    let count = progress
        .journal
        .as_ref()
        .ok_or_else(|| invalid("missing manifest"))?
        .manifest
        .files
        .len();
    for index in 0..count {
        let is_skipped = progress
            .journal
            .as_ref()
            .is_some_and(|journal| journal.completed[index].is_some());
        let result = transfer_file(receiver, directory, progress, &content, index).await;
        if !is_skipped {
            progress.display.file_finished();
        }
        result?;
    }
    Ok(())
}
async fn transfer_file(
    receiver: &mut ShareReceiver,
    directory: &SafeDir,
    progress: &mut Progress,
    content: &ContentClient,
    index: usize,
) -> Result<(), CliError> {
    let journal = progress
        .journal
        .as_ref()
        .ok_or_else(|| invalid("missing manifest"))?;
    if journal.completed[index].is_some() {
        return Ok(());
    }
    let entry = journal.manifest.files[index].clone();
    let is_overwrite = journal.identity.is_overwrite;
    progress.active = Some(index);
    let session = receiver.ensure_session().await?;
    progress.is_issuing = true;
    let issued = session
        .get_file_download_url(&entry.remote.path)
        .await
        .map_err(|error| issuance_error(progress, error))?;
    // [Review Fix #LiveIssuance3] A decoded success with an unusable address is still ambiguous; validate before ending issuance.
    super::share_content::validate_url(&issued.download_url).map_err(|_| {
        issuance_error(
            progress,
            Error::InvalidResponse("unusable issued download address".into()),
        )
    })?;
    progress.is_issuing = false;
    let (staged, digest) = content
        .download(&issued.download_url, &entry.remote, directory, &entry.local)
        .await?;
    staged.publish(is_overwrite)?;
    progress
        .journal
        .as_mut()
        .ok_or_else(|| invalid("missing manifest"))?
        .completed[index] = Some(digest);
    progress.active = None;
    save_progress(progress)
}
fn issuance_error(progress: &mut Progress, error: Error) -> CliError {
    // [Review Fix #LiveIssuance1] Definite service rejection must retain its semantics; metadata success IDs never identify a local failure.
    if let Error::Server(server) = &error {
        progress.failure_status_code = server.status_code;
        progress.failure_request_id = server.request_id.as_deref().and_then(sanitize_request_id);
    }
    let is_known = match &error {
        Error::Server(server) => server
            .status_code
            .is_some_and(|status| status < 500 && status != 408),
        Error::Client(_) | Error::Cli(_) | Error::UnifiedCredential(_) => true,
        _ => false,
    };
    if is_known {
        progress.is_issuing = false;
        map_ids_error(error)
    } else {
        CliError::TransferFailed("[share_download_url_result_unknown] download URL issuance failed; outcome may be unknown; not replayed".into())
    }
}
fn error_kind(error: &CliError) -> ErrorKind {
    // [Review Fix #LiveIssuance2] Aggregate errors must have the same kind as their exit code instead of always claiming transfer failure.
    match error.exit_code() {
        ExitCode::AuthFailed => ErrorKind::AuthFailed,
        ExitCode::ConfigMissing => ErrorKind::ConfigMissing,
        ExitCode::ResourceNotFound => ErrorKind::ResourceNotFound,
        ExitCode::PermissionDenied => ErrorKind::PermissionDenied,
        ExitCode::ValidationError => ErrorKind::ValidationError,
        ExitCode::RateLimited => ErrorKind::RateLimited,
        ExitCode::TransferFailed => ErrorKind::TransferFailed,
        ExitCode::Conflict => ErrorKind::Conflict,
        _ => ErrorKind::Unknown,
    }
}
fn aggregate(progress: &Progress, error: Option<CliError>) -> (Envelope<Value>, i32) {
    // [Review Fix #ShareProgress5] A resumed checkpoint is only a claim until verified; keep unverified entries pending even if preflight/validation is cancelled or fails, without rewriting the disk ledger.
    let files: Vec<_> = progress.journal.iter().flat_map(|journal|journal.manifest.files.iter().enumerate().map(|(index,entry)| {
        let digest = journal.completed[index].as_ref().filter(|_| !progress.is_resuming || progress.verified.get(index) == Some(&true));
        let status = if digest.is_some() {"completed"} else if progress.active==Some(index) {"failed"} else {"pending"};
        json!({"Path":entry.remote.path,"LocalPath":entry.local,"Status":status,"SHA256":digest})
    })).collect();
    let completed = files
        .iter()
        .filter(|file| file["Status"] == "completed")
        .count();
    let failed = files
        .iter()
        .filter(|file| file["Status"] == "failed")
        .count();
    let payload = json!({"Complete":error.is_none(),"PlanComplete":progress.journal.is_some(),"CheckpointAvailable":progress.is_checkpoint_durable,"IssuanceOutcomeUnknown":progress.is_issuing,"Summary":{"Total":files.len(),"Completed":completed,"Failed":failed,"Pending":files.len()-completed-failed,"Skipped":progress.skipped},"Files":files});
    let envelope = Envelope::success("ve-adrive share download", payload).without_request_id();
    let Some(error) = error else {
        return (envelope, 0);
    };
    failed_aggregate(envelope, progress, &error)
}

fn failed_aggregate(
    mut envelope: Envelope<Value>,
    progress: &Progress,
    error: &CliError,
) -> (Envelope<Value>, i32) {
    let code = error.exit_code() as i32;
    let semantics = error.agent_semantics();
    envelope.success = false;
    envelope.status = Status::Failed;
    envelope.status_code = progress.failure_status_code.or(semantics.status_code);
    envelope.request_id = progress.failure_request_id.clone().or(semantics.request_id);
    envelope.ec = Some(semantics.code.clone());
    envelope.error = Some(ErrorDetail {
        status_code: envelope.status_code,
        code: semantics.code,
        message: semantics.message,
        exit_code: code,
        kind: error_kind(error),
        category: semantics.category,
        suggested_action: Some(if progress.is_issuing {
            "Do not automatically retry download URL issuance; inspect the failed file and explicitly resume only after checking its outcome".into()
        } else {
            semantics.suggested_action
        }),
        fix_command: None,
        doctor_hint: None,
        docs_url: None,
    });
    if let Some(detail) = &mut envelope.error {
        apply_download_denial_guidance(detail);
        apply_local_conflict_guidance(detail);
    }
    (envelope, code)
}

fn apply_local_conflict_guidance(detail: &mut ErrorDetail) {
    if matches!(detail.kind, ErrorKind::Conflict)
        && detail.message.contains("[share_local_path_conflict]")
    {
        // [Review Fix #D04] A refused local entry needs filesystem repair, not account or remote permission changes.
        detail.suggested_action = Some("Inspect the local destination; replace the unsafe entry with an ordinary file/directory as appropriate, or choose another directory".into());
        detail.fix_command = None;
        detail.doctor_hint = None;
    }
}

fn apply_download_denial_guidance(detail: &mut ErrorDetail) {
    if detail.code == "Forbidden.ShareDownloadDisabled"
        && matches!(detail.kind, ErrorKind::PermissionDenied)
    {
        // [Review Fix #ShareDownloadDisplay2] A known share policy denial is not repaired by refreshing account or IAM credentials.
        // [Review Fix #D06] This service code can indicate permission or exhausted download count; avoid claiming one cause.
        detail.message =
            "Share download is blocked; share permission or download-count quota may prevent it"
                .into();
        detail.suggested_action = Some("Ask the share owner to check download permission (--permission download) and download-count quota; retry after resolving the restriction".into());
    }
}

#[cfg(test)]
mod tests;
