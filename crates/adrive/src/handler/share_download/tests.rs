use super::*;
use crate::domain::share_access::fixture::{reply, Fixture};

struct Destination(PathBuf);
impl Destination {
    fn new() -> Self {
        let path = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("share-download-{}", ulid::Ulid::new()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn args(&self, endpoint: &str) -> ShareDownloadArgs {
        ShareDownloadArgs {
            share_url: format!("{endpoint}/share/s?pwd=receiver-secret"),
            destination_dir: self.0.join("output").to_string_lossy().into(),
            path: Some("parent/a.txt".into()),
            is_recursive: false,
            is_overwrite: false,
            is_password_stdin: false,
            checkpoint: Some(self.0.join("journal.json").to_string_lossy().into()),
            is_resume: false,
            checkpoint_threshold: None,
            multipart_concurrency: None,
            is_progress: false,
            is_no_progress: false,
        }
    }
}
impl Drop for Destination {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn auth() -> Vec<String> {
    vec![
        reply(200, r#"{"ShareID":"s"}"#),
        reply(200, r#"{"AccessToken":"token-secret","ExpiresIn":60}"#),
    ]
}
fn metadata() -> String {
    reply(
        200,
        r#"{"FilePath":"parent/a.txt","Size":3,"ETag":"abc","UpdatedAt":1}"#,
    )
}
fn content_reply() -> String {
    reply(200, "abc").replace("Connection: close", "ETag: \"abc\"\r\nConnection: close")
}
fn assert_journal_secrets_absent(journal: &str) {
    for secret in [
        "receiver-secret",
        "token-secret",
        "signed-secret",
        "DownloadUrl",
    ] {
        assert!(!journal.contains(secret));
    }
}

#[tokio::test]
async fn explicit_download_denial_retains_permission_diagnostics() {
    let mut responses = auth();
    responses.push(metadata());
    responses.push(
        reply(403, r#"{"Code":"Forbidden.ShareDownloadDisabled"}"#)
            .replace("fixture-id", "denied-request"),
    );
    let service = Fixture::new(responses);
    let destination = Destination::new();
    let mut progress = Progress::default();
    let error = execute(
        &GlobalArgs::default(),
        &destination.args(&service.endpoint),
        &mut progress,
    )
    .await
    .unwrap_err();
    let (envelope, code) = aggregate(&progress, Some(error));
    assert_eq!(code, 5);
    assert_eq!(envelope.status_code, Some(403));
    assert_eq!(
        envelope.ec.as_deref(),
        Some("Forbidden.ShareDownloadDisabled")
    );
    assert_eq!(envelope.request_id.as_deref(), Some("denied-request"));
    assert!(matches!(
        envelope.error.unwrap().kind,
        ErrorKind::PermissionDenied
    ));
    assert_eq!(envelope.data.unwrap()["IssuanceOutcomeUnknown"], false);
    assert!(!destination.0.join("output/a.txt").exists());
    assert_eq!(service.finish().len(), 4);
}

#[test]
fn early_download_denial_gets_owner_guidance_without_changing_other_errors() {
    let denial = "ids server error: [Forbidden.ShareDownloadDisabled] denied (status=403)";
    let permission = CliError::PermissionDenied(denial.into());
    let (envelope, code) = aggregate(&Progress::default(), Some(permission));
    assert_eq!(code, 5);
    let detail = envelope.error.unwrap();
    assert_eq!(detail.status_code, None);
    assert_eq!(
        detail.message,
        "Share download is disabled by the share permission"
    );
    assert!(detail.suggested_action.unwrap().contains("share owner"));

    for error in [
        CliError::PermissionDenied("[Forbidden.ShareRequireLogin] denied".into()),
        CliError::ValidationError(denial.into()),
    ] {
        let (envelope, _) = aggregate(&Progress::default(), Some(error));
        assert!(!envelope
            .error
            .unwrap()
            .message
            .contains("download is disabled"));
    }
}

#[tokio::test]
async fn explicit_rate_limit_is_known_and_never_replayed() {
    let mut responses = auth();
    responses.push(metadata());
    responses.push(reply(429, r#"{"Code":"TooManyRequests"}"#));
    let service = Fixture::new(responses);
    let destination = Destination::new();
    let mut progress = Progress::default();
    let error = execute(
        &GlobalArgs::default(),
        &destination.args(&service.endpoint),
        &mut progress,
    )
    .await
    .unwrap_err();
    let (envelope, code) = aggregate(&progress, Some(error));
    assert_eq!(code, 7);
    assert_eq!(envelope.status_code, Some(429));
    assert!(matches!(
        envelope.error.unwrap().kind,
        ErrorKind::RateLimited
    ));
    assert_eq!(envelope.data.unwrap()["IssuanceOutcomeUnknown"], false);
    assert_eq!(service.finish().len(), 4);
}

#[tokio::test]
async fn invalid_successful_issuance_is_unknown_and_never_replayed() {
    let mut responses = auth();
    responses.push(metadata());
    responses.push(reply(200, "invalid JSON"));
    let service = Fixture::new(responses);
    let destination = Destination::new();
    let mut progress = Progress::default();
    let error = execute(
        &GlobalArgs::default(),
        &destination.args(&service.endpoint),
        &mut progress,
    )
    .await
    .unwrap_err();
    let (envelope, code) = aggregate(&progress, Some(error));
    assert_eq!(code, 8);
    assert_eq!(
        envelope.ec.as_deref(),
        Some("share_download_url_result_unknown")
    );
    assert_eq!(envelope.request_id, None);
    assert!(envelope
        .error
        .unwrap()
        .suggested_action
        .unwrap()
        .contains("Do not automatically retry"));
    assert_eq!(envelope.data.unwrap()["IssuanceOutcomeUnknown"], true);
    assert_eq!(service.finish().len(), 4);
}

#[tokio::test]
async fn unusable_successful_download_addresses_remain_unknown_without_content_requests() {
    // [Review Fix #ShareProtocol5] HTTP is a valid content protocol; only malformed or unsupported addresses should retain issuance ambiguity.
    for address in [
        "",
        "invalid address",
        "ftp://example.test/secret",
        "https://user:secret@example.test/file",
    ] {
        let mut responses = auth();
        responses.push(metadata());
        responses.push(reply(200, &json!({"DownloadURL":address}).to_string()));
        let service = Fixture::new(responses);
        let destination = Destination::new();
        let mut progress = Progress::default();
        let error = execute(
            &GlobalArgs::default(),
            &destination.args(&service.endpoint),
            &mut progress,
        )
        .await
        .unwrap_err();
        let (envelope, code) = aggregate(&progress, Some(error));
        assert_eq!(code, 8);
        assert_eq!(
            envelope.ec.as_deref(),
            Some("share_download_url_result_unknown")
        );
        assert_eq!(
            envelope.data.as_ref().unwrap()["IssuanceOutcomeUnknown"],
            true
        );
        assert!(!serde_json::to_string(&envelope).unwrap().contains("secret"));
        assert!(!destination.0.join("output/a.txt").exists());
        assert_eq!(service.finish().len(), 4);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn checkpoint_alias_cannot_collide_with_download_mapping() {
    let mut responses = auth();
    responses.push(metadata());
    let service = Fixture::new(responses);
    let destination = Destination::new();
    std::fs::create_dir(destination.0.join("output")).unwrap();
    std::os::unix::fs::symlink(destination.0.join("output"), destination.0.join("alias")).unwrap();
    let mut args = destination.args(&service.endpoint);
    args.destination_dir = destination.0.join("alias").to_string_lossy().into();
    args.checkpoint = Some(destination.0.join("output/a.txt").to_string_lossy().into());
    let mut progress = Progress::default();
    let error = execute(&GlobalArgs::default(), &args, &mut progress)
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("checkpoint/lock collides"),
        "{error}"
    );
    assert!(!destination.0.join("output/a.txt").exists());
    assert_eq!(service.finish().len(), 3);
}

#[cfg(unix)]
#[tokio::test]
async fn checkpoint_parent_alias_and_lock_sidecar_cannot_overlap_mapping() {
    for is_lock_collision in [false, true] {
        let mut responses = auth();
        responses.push(if is_lock_collision {
            // [Review Fix #LiveFixture1] Rebuild HTTP framing when the JSON path length changes.
            reply(
                200,
                r#"{"FilePath":"parent/a.txt.lock","Size":3,"ETag":"abc","UpdatedAt":1}"#,
            )
        } else {
            metadata()
        });
        let service = Fixture::new(responses);
        let destination = Destination::new();
        std::fs::create_dir(destination.0.join("output")).unwrap();
        std::os::unix::fs::symlink(destination.0.join("output"), destination.0.join("alias"))
            .unwrap();
        let mut args = destination.args(&service.endpoint);
        args.checkpoint = Some(destination.0.join("alias/a.txt").to_string_lossy().into());
        if is_lock_collision {
            args.path = Some("parent/a.txt.lock".into());
        }
        let mut progress = Progress::default();
        let error = execute(&GlobalArgs::default(), &args, &mut progress)
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("checkpoint/lock collides"),
            "{error}"
        );
        assert!(!destination.0.join("output/a.txt").exists());
        assert_eq!(service.finish().len(), 3);
    }
}
#[tokio::test]
async fn downloads_atomically_and_resume_rechecks_both_sides_without_resigning() {
    let content = Fixture::new(vec![content_reply()]);
    let mut responses = auth();
    responses.push(metadata());
    responses.push(reply(
        200,
        &json!({"DownloadURL":format!("{}/blob?signature=signed-secret",content.endpoint)})
            .to_string(),
    ));
    responses.extend(auth());
    responses.push(metadata());
    let service = Fixture::new(responses);
    let destination = Destination::new();
    let mut args = destination.args(&service.endpoint);
    let mut progress = Progress::default();
    execute(&GlobalArgs::default(), &args, &mut progress)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(destination.0.join("output/a.txt")).unwrap(),
        b"abc"
    );
    let journal = std::fs::read_to_string(args.checkpoint.as_ref().unwrap()).unwrap();
    assert_journal_secrets_absent(&journal);
    drop(progress);
    args.is_resume = true;
    let mut resumed = Progress::default();
    execute(&GlobalArgs::default(), &args, &mut resumed)
        .await
        .unwrap();
    assert_eq!(
        aggregate(&resumed, None).0.data.unwrap()["Summary"]["Completed"],
        1
    );
    let requests = service.finish();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.contains("get_download_url"))
            .count(),
        1
    );
    let requests = content.finish();
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].to_lowercase().contains("authorization:"));
    assert!(!requests[0].to_lowercase().contains("x-share-token:"));
    assert!(!requests[0].to_lowercase().contains("cookie:"));
}
#[tokio::test]
async fn failed_integrity_keeps_checkpoint_pending_and_never_publishes() {
    let content = Fixture::new(vec![reply(200, "wrong")]);
    let mut responses = auth();
    responses.push(metadata());
    responses.push(reply(
        200,
        &json!({"DownloadURL":format!("{}/blob",content.endpoint)}).to_string(),
    ));
    let service = Fixture::new(responses);
    let destination = Destination::new();
    let args = destination.args(&service.endpoint);
    let mut progress = Progress::default();
    let error = execute(&GlobalArgs::default(), &args, &mut progress)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("length differs"), "{error}");
    let (envelope, code) = aggregate(&progress, Some(error));
    assert_ne!(code, 0);
    assert_eq!(envelope.request_id, None);
    assert_eq!(envelope.data.unwrap()["Summary"]["Failed"], 1);
    assert!(!destination.0.join("output/a.txt").exists());
    let journal: Journal =
        serde_json::from_slice(&std::fs::read(args.checkpoint.unwrap()).unwrap()).unwrap();
    assert_eq!(journal.completed, vec![None]);
    assert_eq!(
        std::fs::read_dir(destination.0.join("output"))
            .unwrap()
            .count(),
        0
    );
    service.finish();
    content.finish();
}
#[tokio::test]
async fn all_existing_file_conflicts_are_checked_before_signing() {
    let mut responses = auth();
    responses.push(metadata());
    let service = Fixture::new(responses);
    let destination = Destination::new();
    std::fs::create_dir(destination.0.join("output")).unwrap();
    std::fs::write(destination.0.join("output/a.txt"), "keep").unwrap();
    let args = destination.args(&service.endpoint);
    let mut progress = Progress::default();
    assert!(execute(&GlobalArgs::default(), &args, &mut progress)
        .await
        .is_err());
    assert_eq!(
        std::fs::read(destination.0.join("output/a.txt")).unwrap(),
        b"keep"
    );
    assert!(
        progress.journal.is_some(),
        "preflight failure must retain complete manifest"
    );
    assert_eq!(service.finish().len(), 3);
}
#[tokio::test]
async fn unknown_signing_outcome_is_not_replayed() {
    let mut responses = auth();
    responses.push(metadata());
    responses.push(reply(500, "{}"));
    let service = Fixture::new(responses);
    let destination = Destination::new();
    let args = destination.args(&service.endpoint);
    let mut progress = Progress::default();
    let error = execute(&GlobalArgs::default(), &args, &mut progress)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("outcome may be unknown"));
    assert_eq!(
        aggregate(&progress, Some(error)).0.data.unwrap()["IssuanceOutcomeUnknown"],
        true
    );
    assert_eq!(service.finish().len(), 4);
}

#[tokio::test]
async fn content_retry_reuses_one_signed_address() {
    let content = Fixture::new(vec![reply(503, "busy"), content_reply()]);
    let mut responses = auth();
    responses.push(metadata());
    responses.push(reply(
        200,
        &json!({"DownloadURL":format!("{}/blob?signature=secret",content.endpoint)}).to_string(),
    ));
    let service = Fixture::new(responses);
    let destination = Destination::new();
    let args = destination.args(&service.endpoint);
    let mut progress = Progress::default();
    execute(&GlobalArgs::default(), &args, &mut progress)
        .await
        .unwrap();
    assert_eq!(service.finish().len(), 4);
    let requests = content.finish();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].lines().next(), requests[1].lines().next());
}
#[tokio::test]
async fn local_change_and_remote_change_prevent_resume_before_new_signature() {
    for is_local_change in [true, false] {
        let content = Fixture::new(vec![content_reply()]);
        let mut responses = auth();
        responses.push(metadata());
        responses.push(reply(
            200,
            &json!({"DownloadURL":format!("{}/blob",content.endpoint)}).to_string(),
        ));
        responses.extend(auth());
        responses.push(if is_local_change {
            metadata()
        } else {
            metadata().replace("abc", "xyz")
        });
        let service = Fixture::new(responses);
        let destination = Destination::new();
        let mut args = destination.args(&service.endpoint);
        let mut progress = Progress::default();
        execute(&GlobalArgs::default(), &args, &mut progress)
            .await
            .unwrap();
        drop(progress);
        if is_local_change {
            std::fs::write(destination.0.join("output/a.txt"), b"xyz").unwrap();
        }
        let checkpoint_path = args.checkpoint.as_deref().unwrap();
        let checkpoint_before = std::fs::read(checkpoint_path).unwrap();
        args.is_resume = true;
        let mut progress = Progress::default();
        let error = execute(&GlobalArgs::default(), &args, &mut progress)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("changed since checkpoint"));
        let summary = aggregate(&progress, Some(error)).0.data.unwrap()["Summary"].clone();
        assert_eq!(
            summary["Completed"], 0,
            "an invalid resume must not claim successful files"
        );
        assert_eq!(summary["Skipped"], 0);
        assert_eq!(summary["Failed"], 1);
        assert_eq!(std::fs::read(checkpoint_path).unwrap(), checkpoint_before);
        assert_eq!(service.finish().len(), 7);
        content.finish();
    }
}
#[tokio::test]
async fn dropping_inflight_transfer_cleans_partial_and_preserves_pending_checkpoint() {
    let content = Fixture::new(vec![reply(503, "busy")]);
    let mut responses = auth();
    responses.push(metadata());
    responses.push(reply(
        200,
        &json!({"DownloadURL":format!("{}/blob",content.endpoint)}).to_string(),
    ));
    let service = Fixture::new(responses);
    let destination = Destination::new();
    let args = destination.args(&service.endpoint);
    let mut progress = Progress::default();
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        execute(&GlobalArgs::default(), &args, &mut progress),
    )
    .await;
    assert!(result.is_err());
    assert!(progress.is_checkpoint_durable);
    assert!(!destination.0.join("output/a.txt").exists());
    let journal: Journal =
        serde_json::from_slice(&std::fs::read(args.checkpoint.unwrap()).unwrap()).unwrap();
    assert_eq!(journal.completed, vec![None]);
    service.finish();
    content.finish();
}

#[tokio::test]
async fn known_expiry_renews_once_before_next_file_operation() {
    let mut responses = auth();
    responses.push(reply(
        200,
        r#"{"AccessToken":"renewed-token","ExpiresIn":60}"#,
    ));
    responses.push(metadata());
    let service = Fixture::new(responses);
    let global = GlobalArgs::default();
    let mut receiver =
        ShareReceiver::new(&global, &format!("{}/share/s", service.endpoint)).unwrap();
    let info = receiver.info().await.unwrap();
    receiver.authorize(&global, &info, false).await.unwrap();
    receiver.expires_at = Some(std::time::Instant::now());
    receiver
        .ensure_session()
        .await
        .unwrap()
        .get_file("parent/a.txt")
        .await
        .unwrap();
    let requests = service.finish();
    assert_eq!(requests.len(), 4);
    assert!(requests[3].contains("renewed-token"));
}
#[tokio::test]
async fn partial_failure_preserves_completed_files_and_durable_summary() {
    let content = Fixture::new(vec![content_reply()]);
    let mut responses = auth();
    responses.push(reply(200,r#"{"Files":[{"FilePath":"a","Size":3,"ETag":"abc"},{"FilePath":"b","Size":3,"ETag":"abc"}]}"#));
    responses.push(reply(
        200,
        &json!({"DownloadURL":format!("{}/blob",content.endpoint)}).to_string(),
    ));
    responses.push(reply(500, "{}"));
    let service = Fixture::new(responses);
    let destination = Destination::new();
    let mut args = destination.args(&service.endpoint);
    args.path = None;
    let mut progress = Progress::default();
    let error = execute(&GlobalArgs::default(), &args, &mut progress)
        .await
        .unwrap_err();
    let payload = aggregate(&progress, Some(error)).0.data.unwrap();
    assert_eq!(payload["Summary"]["Completed"], 1);
    assert_eq!(payload["Summary"]["Failed"], 1);
    assert_eq!(
        std::fs::read(destination.0.join("output/a")).unwrap(),
        b"abc"
    );
    assert!(!destination.0.join("output/b").exists());
    let journal: Journal =
        serde_json::from_slice(&std::fs::read(args.checkpoint.unwrap()).unwrap()).unwrap();
    assert!(journal.completed[0].is_some());
    assert!(journal.completed[1].is_none());
    service.finish();
    content.finish();
}
