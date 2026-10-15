//! Preview one shared file without download permissions or implicit browser actions.
use super::{
    common::{map_ids_error, output_result},
    share_receive::ShareReceiver,
};
use crate::{
    cli::{share::ShareViewArgs, ADriveAuthArgs},
    domain::{
        share_access::preview::SharePreviewResult, share_types::ShareListFilesInput,
        types::FileInfo,
    },
};
use serde_json::json;
use std::io::IsTerminal;
use tokio::io::AsyncWriteExt;
use tos_core::agent::{envelope::Envelope, error::CliError, global_args::GlobalArgs};

/// Validate raw output and exact-file syntax without password, config or network I/O.
pub(crate) fn validate(global: &GlobalArgs, args: &ShareViewArgs) -> Result<(), CliError> {
    super::share::validate_selection(args.path.as_deref())?;
    if args.path.as_ref().is_some_and(|path| path.ends_with('/')) {
        return Err(invalid(
            "view requires --path to one exact file, not a directory",
        ));
    }
    if args.is_raw && (global.output.is_some() || global.query.is_some()) {
        return Err(invalid("--raw conflicts with --output and --query"));
    }
    if args.is_raw && std::io::stdout().is_terminal() {
        return Err(invalid(
            "--raw requires a pipe or redirected stdout; try | less",
        ));
    }
    Ok(())
}

/// Execute preview with receiver credentials; raw failures go to stderr only.
pub(crate) async fn view(
    global: &GlobalArgs,
    auth: &ADriveAuthArgs,
    args: &ShareViewArgs,
) -> Result<i32, CliError> {
    validate(global, args)?;
    if !args.is_raw {
        return execute(global, auth, args).await;
    }
    let result = tokio::select! {
        signal = tokio::signal::ctrl_c() => {
            signal.map_err(CliError::Io)?;
            Err(CliError::TransferFailed("share preview interrupted; preview was not retried".into()))
        }
        result = execute(global, auth, args) => result,
    };
    match result {
        Ok(code) => Ok(code),
        Err(error) => {
            eprintln!("{error}");
            Ok(error.exit_code().as_i32())
        }
    }
}

async fn execute(
    global: &GlobalArgs,
    auth: &ADriveAuthArgs,
    args: &ShareViewArgs,
) -> Result<i32, CliError> {
    let mut receiver = ShareReceiver::for_command(global, auth, &args.share_url)?;
    let info = receiver.info().await?;
    receiver
        .authorize(global, &info, args.is_password_stdin)
        .await?;
    let path = select_file(&mut receiver, args.path.as_deref()).await?;
    let result = match receiver
        .ensure_session()
        .await?
        .preview_file(&path, !args.is_raw)
        .await
    {
        Ok(result) => result,
        Err(error) => return preview_failure(global, error, args.is_raw),
    };
    if args.is_raw {
        return stream_raw(result).await;
    }
    let mut value = json!({"FilePath":path, "ShareUrl":receiver.identity.safe_url});
    match result {
        SharePreviewResult::Raw(raw) => {
            value["ResultType"] = json!("raw");
            value["StatusCode"] = json!(raw.response_info.status_code);
            value["Headers"] = json!(raw.response_info.headers);
        }
        SharePreviewResult::Document(document, response_info) => {
            value["ResultType"] = json!("document");
            value["StatusCode"] = json!(response_info.status_code);
            value["PreviewURL"] = json!(document.preview_url);
            value["ExpiresAt"] = json!(document.expires_at);
            value["PreviewType"] = json!(document.preview_type);
        }
    }
    output_result(global, &Envelope::success("ve-adrive share view", value))?;
    Ok(0)
}

async fn select_file(
    receiver: &mut ShareReceiver,
    selection: Option<&str>,
) -> Result<String, CliError> {
    let session = receiver.ensure_session().await?;
    let file = if let Some(path) = selection {
        session.get_file(path).await.map_err(map_ids_error)?
    } else {
        let mut listing = session
            .list_files(&ShareListFilesInput {
                list_root: Some(true),
                limit: Some(2),
                delimiter: Some("/".into()),
                ..Default::default()
            })
            .await
            .map_err(map_ids_error)?;
        if listing.is_truncated
            || !listing.next_marker.is_empty()
            || !listing.folders.is_empty()
            || listing.files.len() != 1
        {
            return Err(invalid(
                "view requires --path to one file when the share has folders or multiple roots",
            ));
        }
        listing.files.remove(0)
    };
    validate_file(file)
}

fn validate_file(file: FileInfo) -> Result<String, CliError> {
    super::share::validate_selection(Some(&file.file_path))?;
    if file.file_type.eq_ignore_ascii_case("folder") || file.file_path.ends_with('/') {
        return Err(invalid("view requires one file, not a directory"));
    }
    Ok(file.file_path)
}

async fn stream_raw(result: SharePreviewResult) -> Result<i32, CliError> {
    let SharePreviewResult::Raw(mut raw) = result else {
        return Err(invalid(
            "raw preview unexpectedly returned a document descriptor",
        ));
    };
    let mut stdout = tokio::io::stdout();
    while let Some(chunk) = raw.response.chunk().await.map_err(|_| {
        CliError::TransferFailed("share preview body failed; preview was not retried".into())
    })? {
        if let Err(error) = stdout.write_all(&chunk).await {
            if error.kind() == std::io::ErrorKind::BrokenPipe {
                return Ok(0);
            }
            return Err(CliError::Io(error));
        }
    }
    match stdout.flush().await {
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(0),
        Err(error) => Err(CliError::Io(error)),
        Ok(()) => Ok(0),
    }
}

fn invalid(message: &str) -> CliError {
    CliError::ValidationError(message.into())
}

fn preview_failure(
    global: &GlobalArgs,
    error: crate::domain::client::Error,
    is_raw: bool,
) -> Result<i32, CliError> {
    use tos_core::agent::envelope::ErrorDetail;
    // [Review Fix #ShareView1] Preserve the real preview status before generic CLI error mapping loses HTTP context.
    let (status, request_id) = match &error {
        crate::domain::client::Error::Server(server) => {
            (server.status_code, server.request_id.clone())
        }
        _ => (None, None),
    };
    let error = map_ids_error(error);
    let semantics = error.agent_semantics();
    let code = error.exit_code().as_i32();
    let detail = ErrorDetail {
        status_code: status,
        code: semantics.code,
        message: semantics.message,
        exit_code: code,
        kind: preview_error_kind(&error),
        category: semantics.category,
        suggested_action: Some(preview_guidance(status).into()),
        fix_command: None,
        doctor_hint: None,
        docs_url: None,
    };
    let mut envelope = Envelope::failed("ve-adrive share view", detail);
    envelope.status_code = status;
    // [Review Fix #ShareView5] Descriptor/stream decoding failures still belong to the terminal preview response, never earlier metadata IDs.
    if let Some(request_id) = tos_core::agent::request_id::select_error_request_id(
        &global.request_trace.snapshot(),
        request_id.as_deref(),
    ) {
        envelope.request_id = Some(request_id);
    }
    if is_raw {
        eprintln!("{}", serde_json::to_string(&envelope)?);
    } else {
        output_result(global, &envelope)?;
    }
    Ok(code)
}

fn preview_error_kind(error: &CliError) -> tos_core::agent::envelope::ErrorKind {
    use tos_core::agent::{envelope::ErrorKind, error::ExitCode};
    match error.exit_code() {
        ExitCode::AuthFailed => ErrorKind::AuthFailed,
        ExitCode::PermissionDenied => ErrorKind::PermissionDenied,
        ExitCode::ResourceNotFound => ErrorKind::ResourceNotFound,
        ExitCode::RateLimited => ErrorKind::RateLimited,
        ExitCode::TransferFailed => ErrorKind::TransferFailed,
        ExitCode::Conflict => ErrorKind::Conflict,
        ExitCode::ValidationError => ErrorKind::ValidationError,
        _ => ErrorKind::Unknown,
    }
}

fn preview_guidance(status: Option<u16>) -> &'static str {
    match status {
        Some(403) => "Check the share owner's preview permission and PreviewLimit",
        Some(404) => "Verify the receiver API endpoint and deployment support for share preview",
        _ => "Preview consumes quota; do not automatically retry an unknown preview outcome",
    }
}
