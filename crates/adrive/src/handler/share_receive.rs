//! Public share receiver runtime with explicit OAuth opt-in.
use super::common::{map_ids_error, output_result, output_result_with_columns};
use super::high_level::{
    adrive_ls_file_entry, adrive_ls_folder_entry, normalize_ls_files_and_folders,
    FILE_TABLE_COLUMNS,
};
use crate::cli::share::*;
use crate::cli::ADriveAuthArgs;
use crate::domain::{
    client::ClientOptions,
    share_access::{ShareClient, ShareSession},
    share_types::{GetShareAccessTokenInput, ShareLinkInfo},
    share_url::{derive_share_api_origin, parse_share_url, ParsedShareUrl},
    types::ListFilesOutput,
};
use serde_json::{json, Value};
use std::fmt;
use std::time::{Duration, Instant};
use tos_core::agent::{
    envelope::{Envelope, PaginationInfo},
    error::CliError,
    global_args::GlobalArgs,
    output::OutputFormat,
};

/// Receiver state keeps its selected client, a bounded secret and an expiring share session.
pub(crate) struct ShareReceiver {
    pub(crate) identity: ParsedShareUrl,
    pub(crate) client: ShareClient,
    pub(crate) session: Option<ShareSession>,
    pub(crate) expires_at: Option<Instant>,
    password: Option<String>,
    pub(crate) auth_context: Option<super::share_receiver_auth::ReceiverContext>,
}
impl fmt::Debug for ShareReceiver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ShareReceiver")
            .field("identity", &self.identity)
            .field("has_password", &self.password.is_some())
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}
impl ShareReceiver {
    /// Build URL-origin receiver transport without loading account config or credentials.
    pub(crate) fn new(global: &GlobalArgs, url: &str) -> Result<Self, CliError> {
        let identity = parse_share_url(url).map_err(map_ids_error)?;
        let endpoint = match &global.endpoint {
            Some(endpoint) => endpoint.clone(),
            None => derive_share_api_origin(&identity.origin).map_err(map_ids_error)?,
        };
        let client = ShareClient::new(endpoint, ClientOptions::default())
            .map_err(map_ids_error)?
            .with_request_trace(global.request_trace.clone())
            .with_redactions(identity.password.iter().cloned().collect());
        Ok(Self {
            identity,
            client,
            session: None,
            expires_at: None,
            password: None,
            auth_context: None,
        })
    }

    /// Select OAuth only for an explicit invocation flag; all other receipt remains account-independent.
    /// Returns selected profile/authentication errors before any receiver request, without fallback.
    pub(crate) fn for_command(
        global: &GlobalArgs,
        auth: &ADriveAuthArgs,
        url: &str,
    ) -> Result<Self, CliError> {
        let mut receiver = Self::new(global, url)?;
        if auth.auth_mode == Some(crate::domain::auth::AuthMode::Oauth) {
            let (client, context) =
                super::share_receiver_auth::oauth_receiver(global, &receiver.identity)?;
            receiver.client = client;
            receiver.auth_context = Some(context);
        }
        Ok(receiver)
    }
    /// Fetch basic public info and reject expired/cancelled shares or anonymous login-required receipt.
    pub(crate) async fn info(&self) -> Result<ShareLinkInfo, CliError> {
        let info = self
            .client
            .get_share_by_anonymous(&self.identity.share_id)
            .await
            .map_err(map_ids_error)?;
        // [Review Fix #ShareIdentity1] Validate anonymous identity before using policy fields or exchanging a token.
        super::share::validate_share_id(&info, &self.identity.share_id)?;
        if info.is_expired == Some(true) {
            return Err(CliError::PermissionDenied(
                "[share_expired] share link expired".into(),
            ));
        }
        if info.is_cancelled == Some(true) {
            return Err(CliError::PermissionDenied(
                "[share_cancelled] share link was cancelled".into(),
            ));
        }
        if info.require_login == Some(true) && !self.client.has_oauth() {
            return Err(CliError::PermissionDenied("[share_login_required] this share requires receiver login; use --auth-mode oauth with the recipient profile".into()));
        }
        Ok(info)
    }
    /// Select one password once, then exchange it without trying alternate secrets on failure.
    pub(crate) async fn authorize(
        &mut self,
        global: &GlobalArgs,
        info: &ShareLinkInfo,
        is_password_stdin: bool,
    ) -> Result<(), CliError> {
        self.password = if is_password_stdin {
            Some(super::share_password::read_password_stdin().await?)
        } else if let Some(password) = &self.identity.password {
            Some(password.clone())
        } else if info.has_password == Some(true) {
            Some(super::share_password::prompt_password(global).await?)
        } else {
            None
        };
        // [Review Fix #SharePassword1] Validate only the chosen source; stdin may override an unused URL password.
        if let Some(password) = &self.password {
            super::share_password::validate_password(password)?;
        }
        self.refresh_session().await
    }
    async fn refresh_session(&mut self) -> Result<(), CliError> {
        let (token, user_id) = self
            .client
            .get_access_token_with_subject(&GetShareAccessTokenInput {
                share_id: self.identity.share_id.clone(),
                share_pwd: self.password.clone(),
                expire_sec: None,
            })
            .await
            .map_err(map_ids_error)?;
        // [Review Fix #ShareReceiveAuth5] The accepted exchange supplies its applied subject; later token renewal must preserve the pinned recipient.
        if let Some(context) = &mut self.auth_context {
            if self.session.is_some() && context.user_id != user_id {
                return Err(CliError::AuthFailed("[share_recipient_changed] receiver identity changed during share token renewal; restart with the intended profile".into()));
            }
            context.user_id = user_id;
        }
        let expiry = Instant::now()
            .checked_add(Duration::from_secs(token.expires_in as u64))
            .ok_or_else(|| CliError::ValidationError("invalid share session lifetime".into()))?;
        // [Review Fix #ShareSecret1] The token and receiver password redact every subsequent service error and request trace.
        self.session = Some(
            self.client
                .session(&token)
                .map_err(map_ids_error)?
                .with_redactions(
                    self.password
                        .iter()
                        .chain(self.identity.password.iter())
                        .cloned()
                        .collect(),
                ),
        );
        self.expires_at = Some(expiry);
        Ok(())
    }
    /// Return a usable session, renewing once only when its known expiry has elapsed.
    /// Unknown request outcomes are never replayed by this method.
    pub(crate) async fn ensure_session(&mut self) -> Result<ShareSession, CliError> {
        if self
            .expires_at
            .is_some_and(|expiry| Instant::now() >= expiry)
        {
            self.refresh_session().await?;
        }
        self.session
            .clone()
            .ok_or_else(|| CliError::ValidationError("share receiver is not authorized".into()))
    }
    fn safe_value(&self, mut value: Value) -> Value {
        super::share::redact_share_payload(&mut value);
        // [Review Fix #ShareSecret3] Business identifiers may equal a password; redact designated secret fields without changing paths or IDs.
        value["ShareUrl"] = json!(self.identity.safe_url);
        value
    }
}

#[cfg(test)]
mod endpoint_tests {
    use super::*;

    #[test]
    fn standard_page_url_selects_api_without_account_configuration() {
        for (page, api) in [
            (
                "http://inst.idsapp.volces.com",
                "http://inst.idsapi.volces.com",
            ),
            (
                "http://inst.idsapp-boe.volces.com:8080",
                "http://inst.idsapi-boe.volces.com:8080",
            ),
            (
                "https://inst.idsapp.volces.com",
                "https://inst.idsapi.volces.com",
            ),
            (
                "https://inst.idsapp-boe.volces.com:8443",
                "https://inst.idsapi-boe.volces.com:8443",
            ),
        ] {
            let receiver = ShareReceiver::new(
                &GlobalArgs::default(),
                &format!("{page}/share/s?pwd=secret"),
            )
            .unwrap();
            assert_eq!(receiver.client.endpoint(), api);
            assert_eq!(receiver.identity.origin, page);
        }
    }

    #[test]
    fn explicit_receiver_api_override_wins_and_custom_page_origin_is_retained() {
        let global = GlobalArgs {
            endpoint: Some("https://custom-api.example.test:8443".into()),
            ..Default::default()
        };
        let receiver =
            ShareReceiver::new(&global, "https://inst.idsapp-boe.volces.com/share/s").unwrap();
        assert_eq!(
            receiver.client.endpoint(),
            "https://custom-api.example.test:8443"
        );
        assert_eq!(
            receiver.identity.safe_url,
            "https://inst.idsapp-boe.volces.com/share/s"
        );
        let receiver =
            ShareReceiver::new(&GlobalArgs::default(), "https://pages.example.test/share/s")
                .unwrap();
        assert_eq!(receiver.client.endpoint(), "https://pages.example.test");
    }
}
async fn inspect_value_with_auth(
    global: &GlobalArgs,
    auth: &ADriveAuthArgs,
    url: &str,
) -> Result<Value, CliError> {
    let receiver = ShareReceiver::for_command(global, auth, url)?;
    // Inspect never exchanges a token or acquires a password; login-required is still basic info.
    let info = receiver
        .client
        .get_share_by_anonymous(&receiver.identity.share_id)
        .await
        .map_err(map_ids_error)?;
    super::share::validate_share_id(&info, &receiver.identity.share_id)?;
    Ok(receiver.safe_value(serde_json::to_value(info)?))
}
async fn files_value_with_auth(
    global: &GlobalArgs,
    auth: &ADriveAuthArgs,
    args: &ShareFilesArgs,
) -> Result<Value, CliError> {
    super::share_listing::validate_arguments(args)?;
    super::share::validate_selection(args.path.as_deref())?;
    let mut receiver = ShareReceiver::for_command(global, auth, &args.share_url)?;
    let info = receiver.info().await?;
    receiver
        .authorize(global, &info, args.is_password_stdin)
        .await?;
    let mut value = if let Some(path) = args.path.as_deref().filter(|path| !path.ends_with('/')) {
        let session = receiver.ensure_session().await?;
        json!({"Files":[session.get_file(path).await.map_err(map_ids_error)?],"Folders":[],"IsTruncated":false})
    } else {
        serde_json::to_value(super::share_listing::list_files(&mut receiver, args).await?)?
    };
    value["ShareInfo"] = serde_json::to_value(info)?;
    value["Selection"] = json!({"Path":args.path,"Marker":args.marker,"MaxKeys":args.max_keys});
    Ok(receiver.safe_value(value))
}
/// Print basic public info without token/password I/O.
pub(crate) async fn inspect(
    global: &GlobalArgs,
    auth: &ADriveAuthArgs,
    args: &InspectShareArgs,
) -> Result<i32, CliError> {
    output_result(
        global,
        &Envelope::success(
            "ve-adrive share inspect",
            inspect_value_with_auth(global, auth, &args.share_url).await?,
        ),
    )?;
    Ok(0)
}
/// Print a bounded current-level listing or one exact file, preserving its continuation marker.
pub(crate) async fn files(
    global: &GlobalArgs,
    auth: &ADriveAuthArgs,
    args: &ShareFilesArgs,
) -> Result<i32, CliError> {
    let mut value = files_value_with_auth(global, auth, args).await?;
    let next_marker = value
        .get("NextMarker")
        .and_then(Value::as_str)
        .filter(|marker| !marker.is_empty())
        .map(str::to_owned);
    let is_default_table = is_default_files_table(global);
    let total_returned = if is_default_table {
        add_files_display_entries(&mut value, args.path.as_deref())?
    } else {
        ["Files", "Folders"]
            .iter()
            .map(|key| value.get(key).and_then(Value::as_array).map_or(0, Vec::len) as u64)
            .sum()
    };
    let envelope =
        Envelope::success("ve-adrive share files", value).with_pagination(PaginationInfo {
            next_token: None,
            next_marker,
            total_returned,
        });
    output_result_with_columns(
        global,
        &envelope,
        is_default_table.then_some(FILE_TABLE_COLUMNS),
    )?;
    Ok(0)
}

fn is_default_files_table(global: &GlobalArgs) -> bool {
    matches!(
        global.output.unwrap_or_else(OutputFormat::auto_detect),
        OutputFormat::Table | OutputFormat::Csv
    ) && !global
        .query
        .as_deref()
        .is_some_and(|query| !query.trim().is_empty())
}

fn add_files_display_entries(value: &mut Value, selection: Option<&str>) -> Result<u64, CliError> {
    let listed: ListFilesOutput = serde_json::from_value(value.clone())?;
    // [Review Fix #ShareFilesRows1] Reuse ls's normalization and row mapping so file/folder markers cannot diverge between the two tables.
    let (files, folders) =
        normalize_ls_files_and_folders(listed.files, listed.folders, selection.unwrap_or(""));
    let entries = folders
        .iter()
        .map(|folder| adrive_ls_folder_entry(folder, false))
        .chain(files.iter().map(|file| adrive_ls_file_entry(file, false)))
        .collect::<Vec<_>>();
    let total_returned = entries.len() as u64;
    value["entries"] = json!(entries);
    Ok(total_returned)
}
#[cfg(test)]
async fn inspect_value(global: &GlobalArgs, url: &str) -> Result<Value, CliError> {
    inspect_value_with_auth(global, &ADriveAuthArgs::default(), url).await
}
#[cfg(test)]
async fn files_value(global: &GlobalArgs, args: &ShareFilesArgs) -> Result<Value, CliError> {
    files_value_with_auth(global, &ADriveAuthArgs::default(), args).await
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::share_access::fixture::{reply, Fixture};
    #[tokio::test]
    async fn anonymous_info_redacts_url_password_before_success_and_failure_trace() {
        for status in [200, 403] {
            let body = if status == 200 {
                r#"{"ShareID":"s"}"#
            } else {
                r#"{"Code":"fixture-url-secret"}"#
            };
            let fixture = Fixture::new(vec![
                reply(status, body).replace("fixture-id", "rid-fixture-url-secret")
            ]);
            let global = GlobalArgs::default();
            let result = inspect_value(
                &global,
                &format!("{}/share/s?pwd=fixture-url-secret", fixture.endpoint),
            )
            .await;
            assert!(!format!("{result:?} {:?}", global.request_trace.snapshot())
                .contains("fixture-url-secret"));
            assert_eq!(result.is_ok(), status == 200);
            assert_eq!(fixture.finish().len(), 1);
        }
    }
    #[tokio::test]
    async fn exact_files_rejects_different_empty_and_missing_file_paths() {
        for body in [r#"{"FilePath":"other"}"#, r#"{"FilePath":""}"#, "{}"] {
            let fixture = Fixture::new(vec![
                reply(200, r#"{"ShareID":"s"}"#),
                reply(200, r#"{"AccessToken":"token-secret","ExpiresIn":60}"#),
                reply(200, body),
            ]);
            let args = ShareFilesArgs {
                share_url: format!("{}/share/s", fixture.endpoint),
                path: Some("requested".into()),
                marker: None,
                max_keys: 1000,
                is_password_stdin: false,
            };
            let error = files_value(&GlobalArgs::default(), &args)
                .await
                .unwrap_err();
            assert!(error.to_string().contains("FilePath"));
            assert!(!error.to_string().contains("token-secret"));
            assert_eq!(fixture.finish().len(), 3);
        }
    }
    #[tokio::test]
    async fn inspect_does_not_validate_an_unused_password() {
        let fixture = Fixture::new(vec![reply(200, r#"{"ShareID":"s"}"#)]);
        let url = format!("{}/share/s?pwd={}", fixture.endpoint, "x".repeat(4097));
        assert!(inspect_value(&GlobalArgs::default(), &url).await.is_ok());
        fixture.finish();
    }
    #[tokio::test]
    async fn anonymous_inspect_rejects_explicit_wrong_or_empty_share_id() {
        for body in [r#"{"ShareID":"other"}"#, r#"{"ShareID":""}"#] {
            let fixture = Fixture::new(vec![reply(200, body)]);
            assert!(inspect_value(
                &GlobalArgs::default(),
                &format!("{}/share/requested", fixture.endpoint)
            )
            .await
            .is_err());
            assert_eq!(fixture.finish().len(), 1);
        }
    }
    #[tokio::test]
    async fn anonymous_files_rejects_share_id_mismatch_before_token_exchange() {
        let fixture = Fixture::new(vec![reply(200, r#"{"ShareID":"other"}"#)]);
        let args = ShareFilesArgs {
            share_url: format!("{}/share/requested", fixture.endpoint),
            path: None,
            marker: None,
            max_keys: 1000,
            is_password_stdin: false,
        };
        assert!(files_value(&GlobalArgs::default(), &args).await.is_err());
        assert_eq!(fixture.finish().len(), 1);
    }
    #[tokio::test]
    async fn inspect_is_anonymous_and_does_not_exchange_token() {
        let fixture = Fixture::new(vec![reply(200, r#"{"ShareID":"s","HasPassword":true}"#)]);
        let result = inspect_value(
            &GlobalArgs::default(),
            &format!("{}/share/s?pwd=never-print", fixture.endpoint),
        )
        .await
        .unwrap();
        assert!(!result.to_string().contains("never-print"));
        let requests = fixture.finish();
        assert_eq!(requests.len(), 1);
        assert!(!requests[0].to_lowercase().contains("authorization:"));
    }
    #[tokio::test]
    async fn exact_file_uses_get_not_prefix_listing() {
        let fixture = Fixture::new(vec![
            reply(200, r#"{"ShareID":"s","HasPassword":true}"#),
            reply(200, r#"{"AccessToken":"token-secret","ExpiresIn":60}"#),
            reply(200, r#"{"FilePath":"a.txt","FileType":"file","Size":1}"#),
        ]);
        let args = ShareFilesArgs {
            share_url: format!("{}/share/s?pwd=secret", fixture.endpoint),
            path: Some("a.txt".into()),
            marker: None,
            max_keys: 1000,
            is_password_stdin: false,
        };
        let result = files_value(&GlobalArgs::default(), &args).await.unwrap();
        assert!(!result.to_string().contains("token-secret"));
        let requests = fixture.finish();
        assert!(requests[2].starts_with("GET /v1/share_file/get?file_path=a.txt"));
        assert!(requests[1].contains("\"SharePwd\":\"secret\""));
    }
    #[tokio::test]
    async fn root_listing_retains_one_page_and_marker() {
        let fixture = Fixture::new(vec![
            reply(200, r#"{"ShareID":"s"}"#),
            reply(200, r#"{"AccessToken":"token","ExpiresIn":60}"#),
            reply(
                200,
                r#"{"Files":[{"FilePath":"a.txt"}],"NextMarker":"next","IsTruncated":true}"#,
            ),
        ]);
        let args = ShareFilesArgs {
            share_url: format!("{}/share/s", fixture.endpoint),
            path: None,
            marker: None,
            max_keys: 1,
            is_password_stdin: false,
        };
        let result = files_value(&GlobalArgs::default(), &args).await.unwrap();
        assert_eq!(result["NextMarker"], "next");
        assert_eq!(result["IsTruncated"], true);
        let requests = fixture.finish();
        assert!(requests[2].contains("list_root=true"));
        assert!(requests[2].contains("delimiter=%2F"));
    }
    #[tokio::test]
    async fn password_matching_file_path_does_not_corrupt_identity() {
        let fixture = Fixture::new(vec![
            reply(200, r#"{"ShareID":"a","HasPassword":true}"#),
            reply(200, r#"{"AccessToken":"token","ExpiresIn":60}"#),
            reply(
                200,
                r#"{"FilePath":"a.txt","FileType":"file","SharePwd":"a"}"#,
            ),
        ]);
        let args = ShareFilesArgs {
            share_url: format!("{}/share/a?pwd=a", fixture.endpoint),
            path: Some("a.txt".into()),
            marker: None,
            max_keys: 1000,
            is_password_stdin: false,
        };
        let result = files_value(&GlobalArgs::default(), &args).await.unwrap();
        assert_eq!(result["Files"][0]["FilePath"], "a.txt");
        assert_eq!(result["ShareInfo"]["ShareID"], "a");
        assert!(result["ShareInfo"].get("SharePwd").is_none());
        fixture.finish();
    }
    #[tokio::test]
    async fn receiver_login_required_never_uses_management_auth() {
        let fixture = Fixture::new(vec![reply(200, r#"{"ShareID":"s","RequireLogin":true}"#)]);
        let args = ShareFilesArgs {
            share_url: format!("{}/share/s", fixture.endpoint),
            path: None,
            marker: None,
            max_keys: 1000,
            is_password_stdin: false,
        };
        let error = files_value(&GlobalArgs::default(), &args)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("share_login_required"));
        assert_eq!(fixture.finish().len(), 1);
    }
}
