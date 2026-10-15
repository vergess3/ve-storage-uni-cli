//! Counted, nonreplayable preview requests with caller-owned raw streams.
use super::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Short-lived document preview capability; Debug intentionally omits the URL.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SharePreviewDocument {
    #[serde(rename = "PreviewURL")]
    pub preview_url: String,
    pub expires_at: DateTime<Utc>,
    pub preview_type: String,
}

impl fmt::Debug for SharePreviewDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharePreviewDocument")
            .field("expires_at", &self.expires_at)
            .field("preview_type", &self.preview_type)
            .finish_non_exhaustive()
    }
}

/// Raw response remains streaming; dropping it closes the unused body.
pub struct SharePreviewRaw {
    pub response: reqwest::Response,
    pub response_info: ResponseInfo,
}

/// Typed preview result, distinguished by the service response header.
pub enum SharePreviewResult {
    Raw(SharePreviewRaw),
    Document(SharePreviewDocument, ResponseInfo),
}

impl ShareSession {
    /// Preview `file_path` once using only this session's share token.
    /// `is_auto` enables document descriptors. Raw results own their response stream.
    /// Returns sanitized validation, transport, service or descriptor errors; never retries.
    pub async fn preview_file(&self, file_path: &str, is_auto: bool) -> Result<SharePreviewResult> {
        validate_required(file_path, "file_path")?;
        if file_path.ends_with('/') {
            return Err(Error::Client("preview requires one exact file path".into()));
        }
        let redactions = self.preview_redactions()?;
        let response = self.send_preview(file_path, is_auto, &redactions).await?;
        let info = preview_info(&response, &redactions);
        let is_document = is_auto
            && response
                .headers()
                .get("x-ids-preview-result")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("document"));
        if !is_document {
            return Ok(SharePreviewResult::Raw(SharePreviewRaw {
                response,
                response_info: info,
            }));
        }
        let bytes = read_bounded_body(response).await?;
        let document: SharePreviewDocument = serde_json::from_slice(&bytes).map_err(|_| {
            Error::InvalidResponse("invalid share document preview response".into())
        })?;
        validate_document(&document)?;
        Ok(SharePreviewResult::Document(document, info))
    }

    fn preview_redactions(&self) -> Result<Vec<String>> {
        let mut secrets = self.client.inner.redactions.clone();
        secrets.extend(self.client.oauth_redactions()?);
        secrets.extend(self.redactions.iter().cloned());
        secrets.push(self.access_token.clone());
        Ok(secrets)
    }

    async fn send_preview(
        &self,
        file_path: &str,
        is_auto: bool,
        redactions: &[String],
    ) -> Result<reqwest::Response> {
        let mut url = self.client.inner.endpoint.clone();
        url.set_path("/v1/share_file/preview");
        url.query_pairs_mut().append_pair("file_path", file_path);
        if is_auto {
            url.query_pairs_mut().append_pair("preview_mode", "auto");
        }
        // Preview issuance consumes quota; even ambiguous failures must never replay.
        let response = self
            .client
            .inner
            .http
            .get(url)
            .header("x-share-token", self.token_header.clone())
            .send()
            .await
            .map_err(|error| {
                self.client.inner.request_trace.record_no_response();
                Error::Http(error.without_url())
            })?;
        let request_id = share_request_id(response.headers(), redactions);
        self.client
            .inner
            .request_trace
            .record_response(request_id.as_deref(), response.status().is_success());
        if !matches!(
            response.status(),
            StatusCode::OK | StatusCode::PARTIAL_CONTENT
        ) {
            let fallback = rejected_fallback(&response, redactions);
            // [Review Fix #ShareView3] A malformed error body must not erase the known refusal status and request ID.
            let rejected = ShareResponse::read(response, redactions)
                .await
                .map_err(|_| fallback)?;
            rejected.check_status(redactions)?;
            return Err(Error::InvalidResponse(
                "unexpected share preview HTTP status".into(),
            ));
        }
        Ok(response)
    }
}

fn rejected_fallback(response: &reqwest::Response, redactions: &[String]) -> Error {
    let info = preview_info(response, redactions);
    Error::Server(IdsError {
        code: info.status_code.to_string(),
        message: "share preview request rejected; error body unavailable".into(),
        request_id: (!info.request_id.is_empty()).then_some(info.request_id),
        status_code: Some(info.status_code),
        response_headers: Some(info.headers),
    })
}

fn preview_info(response: &reqwest::Response, redactions: &[String]) -> ResponseInfo {
    let headers = [
        "content-type",
        "content-length",
        "etag",
        "content-range",
        "accept-ranges",
    ]
    .into_iter()
    .filter_map(|name| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.chars().any(char::is_control))
            .map(|value| (name.to_string(), redact_exact(value, redactions)))
    })
    .collect();
    ResponseInfo {
        status_code: response.status().as_u16(),
        request_id: share_request_id(response.headers(), redactions).unwrap_or_default(),
        headers,
    }
}

fn validate_document(document: &SharePreviewDocument) -> Result<()> {
    let invalid = || Error::InvalidResponse("invalid share document preview descriptor".into());
    if document.preview_url.chars().any(char::is_control) {
        return Err(invalid());
    }
    let url = reqwest::Url::parse(&document.preview_url).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        // [Review Fix #ShareView4] Preserve server renderer types and timestamps without adding unsupported expiry or fragment restrictions.
        || document.preview_type.trim().is_empty()
        || document.preview_type.chars().any(char::is_control)
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::fixture::{reply, Fixture};
    use super::*;
    use serde_json::json;

    fn session(endpoint: String) -> ShareSession {
        ShareClient::new(
            endpoint,
            ClientOptions {
                max_retry_count: Some(3),
                ..Default::default()
            },
        )
        .unwrap()
        .session(&ShareAccessTokenOutput {
            access_token: "preview-secret".into(),
            expires_in: 60,
        })
        .unwrap()
        .with_redactions(vec!["password-secret".into()])
    }

    fn descriptor(address: &str) -> String {
        json!({"PreviewURL":address,"ExpiresAt":"2099-10-15T12:00:00Z","PreviewType":"pdf"})
            .to_string()
    }

    #[test]
    fn document_descriptor_preserves_server_type_time_and_safe_fragments() {
        let document: SharePreviewDocument = serde_json::from_value(json!({
            "PreviewURL":"http://preview.test/session#page=2", "ExpiresAt":"2000-01-01T00:00:00Z", "PreviewType":"future-renderer"
        })).unwrap();
        assert!(validate_document(&document).is_ok());
        assert_eq!(document.preview_type, "future-renderer");
    }

    #[tokio::test]
    async fn auto_decodes_only_explicit_document_result_without_account_auth() {
        let fixture = Fixture::new(vec![reply(
            200,
            &descriptor("https://preview.test/session?key=private"),
        )
        .replace(
            "Connection: close",
            "X-Ids-Preview-Result: DoCuMeNt\r\nConnection: close",
        )]);
        let result = session(fixture.endpoint.clone())
            .preview_file("docs/中文 file.docx", true)
            .await
            .unwrap();
        let SharePreviewResult::Document(document, info) = result else {
            panic!("expected document")
        };
        assert_eq!(document.preview_type, "pdf");
        assert_eq!(info.status_code, 200);
        assert!(!format!("{document:?}").contains("private"));
        let requests = fixture.finish();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].contains("preview_mode=auto"));
        assert!(requests[0].contains("file_path=docs%2F%E4%B8%AD%E6%96%87+file.docx"));
        assert!(requests[0]
            .to_lowercase()
            .contains("x-share-token: preview-secret"));
        assert!(!requests[0].to_lowercase().contains("authorization:"));
    }

    #[tokio::test]
    async fn json_without_result_header_stays_caller_owned_raw_stream() {
        let body = descriptor("https://ordinary-json.test/");
        let fixture = Fixture::new(vec![reply(200, &body)]);
        let result = session(fixture.endpoint.clone())
            .preview_file("a.json", true)
            .await
            .unwrap();
        let SharePreviewResult::Raw(raw) = result else {
            panic!("JSON was misclassified")
        };
        assert_eq!(raw.response.text().await.unwrap(), body);
        assert_eq!(fixture.finish().len(), 1);
    }

    #[tokio::test]
    async fn preview_failures_never_replay_and_redact_secrets() {
        for status in [403, 404, 500, 503, 307] {
            let fixture = Fixture::new(vec![reply(
                status,
                r#"{"Code":"preview-secret","Message":"password-secret"}"#,
            )
            .replace("fixture-id", "rid-preview-secret-password-secret")]);
            let error = session(fixture.endpoint.clone())
                .preview_file("a.txt", true)
                .await
                .err()
                .unwrap();
            let rendered = format!("{error:?} {error}");
            assert!(!rendered.contains("preview-secret") && !rendered.contains("password-secret"));
            assert_eq!(fixture.finish().len(), 1);
        }
    }

    #[tokio::test]
    async fn document_descriptors_reject_unsafe_urls_and_bad_json() {
        for body in [
            descriptor("javascript:preview-secret"),
            descriptor("https://user:secret@preview.test/"),
            "{".into(),
        ] {
            let fixture = Fixture::new(vec![reply(200, &body).replace(
                "Connection: close",
                "X-Ids-Preview-Result: document\r\nConnection: close",
            )]);
            let error = session(fixture.endpoint.clone())
                .preview_file("a.docx", true)
                .await
                .err()
                .unwrap();
            assert!(!format!("{error:?} {error}").contains("preview-secret"));
            assert_eq!(fixture.finish().len(), 1);
        }
    }

    #[tokio::test]
    async fn rejected_preview_retains_status_when_error_body_is_oversized_or_truncated() {
        for status in [403, 404] {
            for length in [128, 9 * 1024 * 1024] {
                let fixture = Fixture::new(vec![format!("HTTP/1.1 {status} Rejected\r\nContent-Length: {length}\r\nx-ids-request-id: refusal-id\r\nConnection: close\r\n\r\n{{")]);
                let error = session(fixture.endpoint.clone())
                    .preview_file("a.txt", true)
                    .await
                    .err()
                    .unwrap();
                let Error::Server(server) = error else {
                    panic!("known HTTP rejection was lost")
                };
                assert_eq!(server.status_code, Some(status));
                assert_eq!(server.request_id.as_deref(), Some("refusal-id"));
                assert_eq!(fixture.finish().len(), 1);
            }
        }
    }

    #[tokio::test]
    async fn raw_request_omits_auto_query_and_retains_partial_status() {
        let fixture = Fixture::new(vec![reply(206, "content")]);
        let result = session(fixture.endpoint.clone())
            .preview_file("a.txt", false)
            .await
            .unwrap();
        let SharePreviewResult::Raw(raw) = result else {
            panic!("expected raw")
        };
        assert_eq!(raw.response_info.status_code, 206);
        assert_eq!(raw.response.text().await.unwrap(), "content");
        let requests = fixture.finish();
        assert!(!requests[0].contains("preview_mode"));
    }
}
