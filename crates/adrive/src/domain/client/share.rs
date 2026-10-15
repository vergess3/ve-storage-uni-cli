//! Authenticated V1 share management, retaining operation-level replay safety.

use super::*;
use crate::domain::share_access::{
    is_retryable_share_error, share_request_id, ShareRequest, ShareResponse, ShareRetryPolicy,
};
use crate::domain::share_types::*;
use std::sync::atomic::Ordering;

/// Build the cached share-only transport using the configured timeout and pool options.
/// Returns an HTTP builder error; legacy Resource redirect behavior remains independent.
pub(super) fn build_share_http(options: &ClientOptions) -> Result<reqwest::Client> {
    // [Review Fix #6] A 307/308 redirect can replay a write outside the application retry gate.
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(user_agent())
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(
            options
                .connecttimeout
                .unwrap_or(DEFAULT_HTTP_CONNECT_TIMEOUT_SECONDS),
        ))
        .timeout(Duration::from_secs(
            options
                .requesttimeout
                .unwrap_or(DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS),
        ))
        .pool_max_idle_per_host(
            options
                .maxconnections
                .unwrap_or(DEFAULT_HTTP_MAX_CONNECTIONS),
        )
        .build()
        .map_err(|error| Error::Http(error.without_url()))
}

struct ShareAttempt {
    response: ShareResponse,
    oauth_access_token: Option<String>,
}

struct AppliedShareAuth {
    access_token: Option<String>,
    user_id: Option<String>,
    redactions: AppliedRedactionContext,
}

impl Client {
    /// Build an OAuth receiver client bound to this configured API service.
    /// `page_origin` checks known standard instance labels before any request.
    /// Returns auth/binding/origin errors without exposing or acquiring a token.
    pub(crate) fn receiver_share_client(
        &self,
        page_origin: &str,
    ) -> Result<crate::domain::share_access::ShareClient> {
        let RequestAuth::OAuth(manager) = &self.inner.auth else {
            return Err(Error::Client(
                "share OAuth receipt requires an OAuth client".into(),
            ));
        };
        let instance = crate::domain::share_url::standard_share_instance(page_origin)?;
        if let Some(instance) = &instance {
            manager.validate_instance(instance)?;
        }
        crate::domain::share_access::ShareClient::from_oauth(
            self.clone(),
            self.inner.share_http.clone(),
            self.inner.max_retry_count,
            instance,
        )
    }

    /// Send only public share metadata or token exchange with the selected OAuth identity.
    /// Retains existing bounded retries/refresh and sanitized errors; other paths are rejected.
    pub(crate) async fn send_receiver_share(&self, request: ShareRequest) -> Result<ShareResponse> {
        if !matches!(self.inner.auth, RequestAuth::OAuth(_))
            || !matches!(
                request.path,
                "/v1/share_link/get_by_anonymous" | "/v1/share_link/get_access_token"
            )
        {
            return Err(Error::Client(
                "invalid authenticated share receiver request".into(),
            ));
        }
        self.share_send(request).await
    }

    /// Return the configured API endpoint for deriving a separate share-page origin.
    pub(crate) fn endpoint(&self) -> &str {
        &self.inner.endpoint
    }

    /// Attach an invocation-scoped flag marking authenticated management-write send attempts.
    /// Cloned clients share the flag; validation, credential waits and metadata never mark it.
    pub(crate) fn with_share_submission_observer(mut self, observer: Arc<AtomicBool>) -> Self {
        Arc::make_mut(&mut self.inner).share_submission_observer = Some(observer);
        self
    }

    /// Create one validated file/folder share with account authentication.
    /// Returns metadata or sanitized errors; unknown submissions are never replayed.
    pub async fn create_share_link(&self, input: &CreateShareLinkInput) -> Result<ShareLinkInfo> {
        input.validate()?;
        self.validate_share_instance(&input.instance_id)?;
        let request = ShareRequest::mutation(
            Method::POST,
            "/v1/share_link/create",
            input,
            input.share_pwd.iter().cloned().collect(),
        )?;
        self.share_json(request).await
    }

    /// Fetch management metadata for a nonempty ShareID in the selected Instance.
    /// Returns sanitized validation, account-auth, service or JSON errors.
    pub async fn get_share_link(&self, input: &GetShareLinkInput) -> Result<ShareLinkInfo> {
        self.validate_share_instance(&input.instance_id)?;
        validate_required(&input.share_id, "ShareID")?;
        self.share_json(ShareRequest::read(
            "/v1/share_link/get",
            vec![("share_id".into(), input.share_id.clone())],
        ))
        .await
    }

    /// Update explicit optional fields with the service's optimistic `UpdatedAt` value.
    /// Returns metadata or sanitized errors; unknown submissions are never replayed.
    pub async fn update_share_link(&self, input: &UpdateShareLinkInput) -> Result<ShareLinkInfo> {
        input.validate()?;
        self.validate_share_instance(&input.instance_id)?;
        let request = ShareRequest::mutation(
            Method::PUT,
            "/v1/share_link/update",
            input,
            input.share_pwd.iter().cloned().collect(),
        )?;
        self.share_json(request).await
    }

    /// Cancel a share once, accepting success without a response body.
    /// Returns response metadata or sanitized errors; cancellation is never replayed.
    pub async fn cancel_share_link(&self, input: &CancelShareLinkInput) -> Result<ResponseInfo> {
        self.validate_share_instance(&input.instance_id)?;
        validate_required(&input.share_id, "ShareID")?;
        let request =
            ShareRequest::mutation(Method::POST, "/v1/share_link/cancel", input, Vec::new())?;
        let response = self.share_send(request).await?;
        response.check_status(&[])?;
        Ok(response.info())
    }

    /// Search shares with mandatory creators and validated timestamps/pagination.
    /// Returns preserved service page fields or sanitized errors; metadata may retry.
    pub async fn search_share_links(
        &self,
        input: &SearchShareLinksInput,
    ) -> Result<SearchShareLinksOutput> {
        input.validate()?;
        self.validate_share_instance(&input.instance_id)?;
        self.share_json(ShareRequest::read(
            "/v1/share_link/search",
            input.to_query_pairs(),
        ))
        .await
    }

    fn validate_share_instance(&self, instance_id: &str) -> Result<()> {
        validate_required(instance_id, "InstanceID")?;
        if let RequestAuth::OAuth(manager) = &self.inner.auth {
            manager.validate_instance(instance_id)?;
        }
        Ok(())
    }

    async fn share_json<T: DeserializeOwned + HasResponseInfo>(
        &self,
        request: ShareRequest,
    ) -> Result<T> {
        let response = self.share_send(request).await?;
        let mut output: T = response.decode(&[])?;
        output.set_response_info(response.info());
        Ok(output)
    }

    async fn share_send(&self, mut request: ShareRequest) -> Result<ShareResponse> {
        let first = self.share_send_retries(&request).await?;
        if first.response.status != StatusCode::UNAUTHORIZED {
            return Ok(first.response);
        }
        let RequestAuth::OAuth(manager) = &self.inner.auth else {
            return Ok(first.response);
        };
        let token = first.oauth_access_token.as_deref().ok_or_else(|| {
            Error::InvalidResponse("OAuth share request did not record its access token".into())
        })?;
        // [Review Fix #ShareReceiveAuth3] Keep the rejected credential redacted after refresh, including later share-session metadata.
        request
            .redactions
            .extend(first.response.redaction_values().iter().cloned());
        manager.force_refresh(token).await?;
        // A concrete 401 proves rejection; only the forced authentication refresh may replay it.
        Ok(self.share_send_retries(&request).await?.response)
    }

    async fn share_send_retries(&self, request: &ShareRequest) -> Result<ShareAttempt> {
        for attempt in 0..=self.inner.max_retry_count.min(8) {
            let can_retry = request.retry_policy == ShareRetryPolicy::Metadata
                && attempt < self.inner.max_retry_count.min(8);
            match self.share_send_once(request).await {
                Ok(applied)
                    if can_retry && should_retry_storage_status(applied.response.status, true) => {}
                Ok(applied) => return Ok(applied),
                Err(error) if can_retry && is_retryable_share_error(&error) => {}
                Err(error) => return Err(error),
            }
            sleep_before_retry(attempt).await;
        }
        Err(Error::InvalidResponse("share retry loop exhausted".into()))
    }

    async fn share_send_once(&self, request: &ShareRequest) -> Result<ShareAttempt> {
        let query = (!request.query.is_empty()).then_some(&request.query);
        let url = self.request_url(request.path, query)?;
        let mut headers = HeaderMap::new();
        let content_type = request.body.as_ref().map(|_| "application/json");
        apply_content_headers(&url, &mut headers, content_type, None)?;
        let auth = self
            .apply_share_request_auth(request, &url, &mut headers)
            .await?;
        let mut redactions = auth.redactions;
        redactions
            .exact_values
            .extend(request.redactions.iter().cloned());
        self.share_static_redactions(&headers, &mut redactions);
        let mut builder = self
            .inner
            .share_http
            .request(request.method.clone(), url)
            .headers(headers);
        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }
        // [Review Fix #ShareSubmissionBoundary2] Credential refresh is not a share submission; mark only after authentication at the non-replayable send boundary.
        if request.retry_policy == ShareRetryPolicy::NonReplayable {
            if let Some(observer) = &self.inner.share_submission_observer {
                observer.store(true, Ordering::SeqCst);
            }
        }
        let response =
            self.finish_request(builder.send().await.map_err(reqwest::Error::without_url))?;
        let request_id = share_request_id(response.headers(), &redactions.exact_values);
        self.inner
            .request_trace
            .record_response(request_id.as_deref(), response.status().is_success());
        let mut response = ShareResponse::read(response, &redactions.exact_values).await?;
        response.oauth_user_id = auth.user_id;
        Ok(ShareAttempt {
            response,
            oauth_access_token: auth.access_token,
        })
    }

    async fn apply_share_request_auth(
        &self,
        request: &ShareRequest,
        url: &reqwest::Url,
        headers: &mut HeaderMap,
    ) -> Result<AppliedShareAuth> {
        if let RequestAuth::OAuth(manager) = &self.inner.auth {
            // [Review Fix #ShareReceiveAuth5] Bind recovery to the same credential snapshot that authenticates the accepted exchange, never a later profile read.
            let (access_token, user_id) = manager
                .access_token_and_user_id(true, request.receiver_instance.as_deref())
                .await?;
            let mut authorization = HeaderValue::from_str(&format!("Bearer {access_token}"))
                .map_err(|_| Error::client("invalid OAuth Authorization header"))?;
            authorization.set_sensitive(true);
            headers.insert(reqwest::header::AUTHORIZATION, authorization);
            return Ok(AppliedShareAuth {
                access_token: Some(access_token.clone()),
                user_id,
                redactions: AppliedRedactionContext::from_values([access_token]),
            });
        }
        let query = (!request.query.is_empty()).then_some(&request.query);
        let (access_token, redactions) = self
            .apply_request_auth(&request.method, request.path, query, url, headers)
            .await?;
        Ok(AppliedShareAuth {
            access_token,
            user_id: None,
            redactions,
        })
    }

    fn share_static_redactions(
        &self,
        headers: &HeaderMap,
        redactions: &mut AppliedRedactionContext,
    ) {
        if let Some(authorization) = headers
            .get(reqwest::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
        {
            redactions.exact_values.push(authorization.into());
        }
        if let RequestAuth::Aksk(auth) = &self.inner.auth {
            redactions
                .exact_values
                .extend([auth.access_key.clone(), auth.secret_key.clone()]);
            redactions
                .exact_values
                .extend(auth.security_token.iter().cloned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::share_access::fixture::{reply, Fixture};
    use serde_json::{json, Value};

    struct BlockedRefreshFixture {
        endpoint: String,
        seen: tokio::sync::oneshot::Receiver<()>,
        release: Option<std::sync::mpsc::Sender<()>>,
        worker: Option<std::thread::JoinHandle<()>>,
    }
    impl BlockedRefreshFixture {
        fn new() -> Self {
            use std::io::Write;
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            listener.set_nonblocking(true).unwrap();
            let (signal, seen) = tokio::sync::oneshot::channel();
            let (release, wait) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                let deadline = std::time::Instant::now() + Duration::from_secs(3);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && std::time::Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(error) => panic!("auth fixture accept failed: {error}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let request = crate::domain::share_access::fixture::read_request(&mut stream);
                assert!(request.starts_with("POST /v1/oauth/token "));
                signal
                    .send(())
                    .expect("observer test must still await auth");
                // Fixture cancellation closes the gate; a deadline also releases it instead of hanging a failed test.
                wait.recv_timeout(Duration::from_secs(3))
                    .unwrap_or_default();
                if let Err(error) =
                    stream.write_all(reply(400, r#"{"error":"invalid_grant"}"#).as_bytes())
                {
                    assert!(matches!(
                        error.kind(),
                        std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                    ));
                }
            });
            Self {
                endpoint,
                seen,
                release: Some(release),
                worker: Some(worker),
            }
        }
    }
    impl Drop for BlockedRefreshFixture {
        fn drop(&mut self) {
            self.release.take();
            if let Some(worker) = self.worker.take() {
                if worker.join().is_err() {
                    eprintln!("blocked auth fixture worker failed");
                }
            }
        }
    }

    fn observer_oauth_client(
        auth_endpoint: &str,
        resource_endpoint: &str,
        is_expired: bool,
    ) -> (std::path::PathBuf, Client) {
        use tos_core::infra::credentials::{CredentialsFile, StoredOAuthCredentials};
        let (directory, manager) = oauth_manager("i");
        let path = directory.join("credentials.toml");
        let mut credentials = CredentialsFile::load_from(&path).unwrap();
        credentials
            .set_adrive_oauth(
                "default",
                StoredOAuthCredentials {
                    access_token: Some("access".into()),
                    refresh_token: Some("refresh".into()),
                    expires_at: Some(
                        (Utc::now() + chrono::Duration::hours(if is_expired { -1 } else { 1 }))
                            .to_rfc3339(),
                    ),
                    instance_id: Some("i".into()),
                    auth_endpoint: Some(auth_endpoint.into()),
                    ..Default::default()
                },
            )
            .unwrap();
        credentials.save_to_path(&path).unwrap();
        let client = Client::new_oauth(
            manager,
            Some(resource_endpoint.into()),
            Some("region".into()),
            ClientOptions {
                max_retry_count: Some(0),
                ..Default::default()
            },
        )
        .unwrap();
        (directory, client)
    }

    async fn assert_observer_while_refresh_waits(is_initial_refresh: bool) {
        let mut auth = BlockedRefreshFixture::new();
        let resource = Fixture::new(if is_initial_refresh {
            vec![]
        } else {
            vec![reply(401, "{}")]
        });
        let (directory, client) =
            observer_oauth_client(&auth.endpoint, &resource.endpoint, is_initial_refresh);
        let observer = Arc::new(AtomicBool::new(false));
        let client = client.with_share_submission_observer(observer.clone());
        let input = UpdateShareLinkInput {
            instance_id: "i".into(),
            share_id: "s".into(),
            updated_at: 1,
            share_name: Some("New".into()),
            ..Default::default()
        };
        let mut request = Box::pin(client.update_share_link(&input));
        tokio::select! {
            result=&mut request => panic!("operation finished before auth barrier: success={}",result.is_ok()),
            result=&mut auth.seen => result.unwrap(),
        }
        assert_eq!(observer.load(Ordering::SeqCst), !is_initial_refresh);
        drop(request);
        drop(auth);
        assert_eq!(resource.finish().len(), usize::from(!is_initial_refresh));
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[tokio::test]
    async fn share_submission_observer_stays_false_during_initial_auth_wait() {
        assert_observer_while_refresh_waits(true).await;
    }
    #[tokio::test]
    async fn share_submission_observer_stays_true_during_refresh_after_write_401() {
        assert_observer_while_refresh_waits(false).await;
    }
    #[tokio::test]
    async fn share_submission_observer_ignores_binding_rejection() {
        let (directory, manager) = oauth_manager("bound");
        let observer = Arc::new(AtomicBool::new(false));
        let client = Client::new_oauth(
            manager,
            Some("https://resource.example.test".into()),
            Some("region".into()),
            ClientOptions::default(),
        )
        .unwrap()
        .with_share_submission_observer(observer.clone());
        assert!(client
            .cancel_share_link(&CancelShareLinkInput {
                instance_id: "other".into(),
                share_id: "s".into()
            })
            .await
            .is_err());
        assert!(!observer.load(Ordering::SeqCst));
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[tokio::test]
    async fn share_submission_observer_ignores_metadata_and_marks_cloned_client_write() {
        let fixture = Fixture::new(vec![
            reply(200, "{}"),
            reply(200, r#"{"Items":[]}"#),
            reply(204, ""),
        ]);
        let observer = Arc::new(AtomicBool::new(false));
        let client =
            client(fixture.endpoint.clone()).with_share_submission_observer(observer.clone());
        client
            .get_share_link(&GetShareLinkInput {
                instance_id: "i".into(),
                share_id: "s".into(),
            })
            .await
            .unwrap();
        assert!(!observer.load(Ordering::SeqCst));
        client
            .search_share_links(&SearchShareLinksInput {
                instance_id: "i".into(),
                creators: vec!["u".into()],
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!observer.load(Ordering::SeqCst));
        client
            .clone()
            .cancel_share_link(&CancelShareLinkInput {
                instance_id: "i".into(),
                share_id: "s".into(),
            })
            .await
            .unwrap();
        assert!(observer.load(Ordering::SeqCst));
        assert_eq!(fixture.finish().len(), 3);
    }

    fn client(endpoint: String) -> Client {
        Client::new(
            "ak".into(),
            "sk-secret".into(),
            None,
            Some(endpoint),
            Some("region".into()),
            ClientOptions {
                max_retry_count: Some(2),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn oauth_manager(instance_id: &str) -> (std::path::PathBuf, OAuthTokenManager) {
        use tos_core::infra::credentials::{CredentialsFile, StoredOAuthCredentials};
        let directory =
            std::env::temp_dir().join(format!("adrive-share-oauth-{}", ulid::Ulid::new()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("credentials.toml");
        let mut credentials = CredentialsFile::default();
        credentials
            .set_adrive_oauth(
                "default",
                StoredOAuthCredentials {
                    access_token: Some("access".into()),
                    refresh_token: Some("refresh".into()),
                    expires_at: Some((Utc::now() + chrono::Duration::hours(1)).to_rfc3339()),
                    instance_id: Some(instance_id.into()),
                    auth_endpoint: Some("https://auth.example.test".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        credentials.save_to_path(&path).unwrap();
        let manager =
            OAuthTokenManager::new(path, "default".into(), ClientOptions::default()).unwrap();
        (directory, manager)
    }

    // [Review Fix #8] Separate fixture actions and assertions to keep every test helper under 50 lines.
    async fn exercise_management_routes(client: &Client) {
        client
            .create_share_link(&CreateShareLinkInput {
                instance_id: "i".into(),
                space_id: "sp".into(),
                file_path_list: vec!["a".into()],
                permission: Some("download".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        client
            .get_share_link(&GetShareLinkInput {
                instance_id: "i".into(),
                share_id: "s".into(),
            })
            .await
            .unwrap();
        client
            .update_share_link(&UpdateShareLinkInput {
                instance_id: "i".into(),
                share_id: "s".into(),
                updated_at: 42,
                expiration_sec: Some(0),
                share_pwd: Some(String::new()),
                ..Default::default()
            })
            .await
            .unwrap();
        client
            .search_share_links(&SearchShareLinksInput {
                instance_id: "i".into(),
                creators: vec!["u1".into(), "u2".into()],
                ..Default::default()
            })
            .await
            .unwrap();
        client
            .cancel_share_link(&CancelShareLinkInput {
                instance_id: "i".into(),
                share_id: "s".into(),
            })
            .await
            .unwrap();
    }

    fn assert_management_requests(requests: &[String]) {
        assert!(requests[0].starts_with("POST /v1/share_link/create "));
        let body: Value =
            serde_json::from_str(requests[0].split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(
            body,
            json!({"InstanceID":"i","SpaceID":"sp","FilePathList":["a"],"Permission":"download"})
        );
        assert!(requests[1].starts_with("GET /v1/share_link/get?share_id=s "));
        assert!(requests[2].starts_with("PUT /v1/share_link/update "));
        let body: Value =
            serde_json::from_str(requests[2].split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(
            body,
            json!({"ShareID":"s","UpdatedAt":42,"ExpirationSec":0,"SharePwd":""})
        );
        assert!(requests[3].contains("instance_id=i&creators=u1&creators=u2"));
        assert!(requests[4].starts_with("POST /v1/share_link/cancel "));
        assert!(requests
            .iter()
            .all(|request| request.to_ascii_lowercase().contains("authorization:")));
    }

    #[tokio::test]
    async fn management_routes_bodies_and_repeated_creators_match_sdk() {
        let fixture = Fixture::new(vec![
            reply(200, "{\"ShareID\":\"s\"}"),
            reply(200, "{}"),
            reply(200, "{}"),
            reply(200, "{\"Items\":[],\"Total\":0}"),
            reply(204, ""),
        ]);
        let client = client(fixture.endpoint.clone());
        exercise_management_routes(&client).await;
        assert_management_requests(&fixture.finish());
    }

    #[tokio::test]
    async fn share_mutations_never_replay_after_server_error() {
        for operation in ["create", "update", "cancel"] {
            let fixture = Fixture::new(vec![reply(500, "{\"Message\":\"pw-secret sk-secret\"}")]);
            let client = client(fixture.endpoint.clone());
            let result = match operation {
                "create" => client
                    .create_share_link(&CreateShareLinkInput {
                        instance_id: "i".into(),
                        space_id: "sp".into(),
                        file_path_list: vec!["a".into()],
                        share_pwd: Some("pw-secret".into()),
                        ..Default::default()
                    })
                    .await
                    .map(|_| ()),
                "update" => client
                    .update_share_link(&UpdateShareLinkInput {
                        instance_id: "i".into(),
                        share_id: "s".into(),
                        updated_at: 1,
                        share_pwd: Some("pw-secret".into()),
                        ..Default::default()
                    })
                    .await
                    .map(|_| ()),
                _ => client
                    .cancel_share_link(&CancelShareLinkInput {
                        instance_id: "i".into(),
                        share_id: "s".into(),
                    })
                    .await
                    .map(|_| ()),
            };
            let error = result.unwrap_err();
            assert!(!format!("{error:?} {error}").contains("sk-secret"));
            if operation != "cancel" {
                assert!(!format!("{error:?} {error}").contains("pw-secret"));
            }
            assert_eq!(fixture.finish().len(), 1);
        }
    }
    // [Review Fix #8] Name incomplete-body fixtures so the retry assertion stays short and readable.
    fn incomplete_share_responses() -> Vec<String> {
        vec![
            "HTTP/1.1 200 Fixture\r\nContent-Length: 20\r\nConnection: close\r\n\r\n{".into(),
            "HTTP/1.1 200 Fixture\r\nContent-Length: 24\r\nConnection: close\r\n\r\n{\"ShareID\":\"pw-secret\"}".into(),
        ]
    }

    #[tokio::test]
    async fn share_mutations_never_replay_on_truncated_or_invalid_json_body() {
        for response in incomplete_share_responses() {
            let fixture = Fixture::new(vec![response]);
            let client = client(fixture.endpoint.clone());
            let result = client
                .create_share_link(&CreateShareLinkInput {
                    instance_id: "i".into(),
                    space_id: "sp".into(),
                    file_path_list: vec!["a".into()],
                    share_pwd: Some("pw-secret".into()),
                    ..Default::default()
                })
                .await;
            assert!(result.is_err());
            assert_eq!(fixture.finish().len(), 1);
        }
    }

    #[tokio::test]
    async fn share_oauth_binding_is_checked_before_routes_without_instance_path() {
        let (directory, manager) = oauth_manager("bound");
        let client = Client::new_oauth(
            manager,
            Some("https://api.example.test".into()),
            Some("region".into()),
            ClientOptions::default(),
        )
        .unwrap();
        let result = client
            .get_share_link(&GetShareLinkInput {
                instance_id: "other".into(),
                share_id: "s".into(),
            })
            .await;
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("different ADrive Instance"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn management_errors_redact_unified_secret_keys() {
        let fixture = Fixture::new(vec![reply(403, "{\"Code\":\"UNIFIED_SK_SECRET\"}")]);
        let client = Client::new_unified_for_test(
            || {
                Ok(UnifiedCredentialValue::new(
                    "UNIFIED_AK",
                    "UNIFIED_SK_SECRET",
                    "",
                    "test",
                ))
            },
            Some(fixture.endpoint.clone()),
            Some("region".into()),
            ClientOptions::default(),
        )
        .unwrap();
        let error = client
            .get_share_link(&GetShareLinkInput {
                instance_id: "i".into(),
                share_id: "s".into(),
            })
            .await
            .unwrap_err();
        assert!(!format!("{error:?} {error}").contains("UNIFIED_SK_SECRET"));
        assert_eq!(fixture.finish().len(), 1);
    }
    #[tokio::test]
    async fn authenticated_share_mutations_do_not_follow_307_or_308_redirects() {
        for status in [307, 308] {
            let destination = Fixture::new(vec![reply(200, "{\"ShareID\":\"replayed\"}")]);
            let location = format!("{}/v1/share_link/create", destination.endpoint);
            let source = Fixture::new(vec![format!(
                "HTTP/1.1 {status} Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )]);
            let result = client(source.endpoint.clone())
                .create_share_link(&CreateShareLinkInput {
                    instance_id: "i".into(),
                    space_id: "sp".into(),
                    file_path_list: vec!["a".into()],
                    ..Default::default()
                })
                .await;
            let submitted = source.finish();
            let redirected = destination.finish();
            assert_eq!(submitted.len(), 1);
            assert!(submitted[0].to_ascii_lowercase().contains("authorization:"));
            assert!(
                redirected.is_empty(),
                "HTTP {status} replayed the authenticated submission to another origin"
            );
            assert!(result.is_err());
        }
    }
}
