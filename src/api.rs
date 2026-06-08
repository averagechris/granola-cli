use crate::error::CliError;
use crate::types::{ListFoldersResponse, ListNotesResponse, Note};
use reqwest::{header, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "https://public-api.granola.ai";

#[derive(Debug, Clone)]
pub struct GranolaClient {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl GranolaClient {
    pub fn new(api_key: String) -> Result<Self, CliError> {
        Self::with_base_url(DEFAULT_BASE_URL.to_string(), api_key)
    }

    #[cfg(test)]
    fn with_base_url(base_url: String, api_key: String) -> Result<Self, CliError> {
        let http = reqwest::Client::builder()
            .user_agent(format!("granola-cli/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()?;

        Ok(Self {
            base_url,
            api_key,
            http,
        })
    }

    pub async fn list_notes(
        &self,
        params: &ListNotesParams,
    ) -> Result<ListNotesResponse, CliError> {
        self.get_json("/v1/notes", params.to_query()).await
    }

    pub async fn get_note(
        &self,
        note_id: &str,
        include_transcript: bool,
    ) -> Result<Note, CliError> {
        let mut query = Vec::new();
        if include_transcript {
            query.push(("include", "transcript".to_string()));
        }
        self.get_json(&format!("/v1/notes/{note_id}"), query).await
    }

    pub async fn list_folders(
        &self,
        params: &ListFoldersParams,
    ) -> Result<ListFoldersResponse, CliError> {
        self.get_json("/v1/folders", params.to_query()).await
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Vec<(&'static str, String)>,
    ) -> Result<T, CliError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .get(url)
            .bearer_auth(self.api_key.trim())
            .header(header::ACCEPT, "application/json")
            .query(&query)
            .send()
            .await?;

        let status = response.status();
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());
        let body = response.text().await.unwrap_or_default();

        if status.is_success() {
            return serde_json::from_str(&body).map_err(CliError::from);
        }

        Err(http_error(status, body, retry_after))
    }
}

#[derive(Debug, Clone, Default)]
pub struct ListNotesParams {
    pub created_before: Option<String>,
    pub created_after: Option<String>,
    pub updated_after: Option<String>,
    pub folder_id: Option<String>,
    pub cursor: Option<String>,
    pub page_size: Option<u8>,
}

impl ListNotesParams {
    fn to_query(&self) -> Vec<(&'static str, String)> {
        let mut query = Vec::new();
        push_opt(&mut query, "created_before", &self.created_before);
        push_opt(&mut query, "created_after", &self.created_after);
        push_opt(&mut query, "updated_after", &self.updated_after);
        push_opt(&mut query, "folder_id", &self.folder_id);
        push_opt(&mut query, "cursor", &self.cursor);
        if let Some(page_size) = self.page_size {
            query.push(("page_size", page_size.to_string()));
        }
        query
    }
}

#[derive(Debug, Clone, Default)]
pub struct ListFoldersParams {
    pub cursor: Option<String>,
    pub page_size: Option<u8>,
}

impl ListFoldersParams {
    fn to_query(&self) -> Vec<(&'static str, String)> {
        let mut query = Vec::new();
        push_opt(&mut query, "cursor", &self.cursor);
        if let Some(page_size) = self.page_size {
            query.push(("page_size", page_size.to_string()));
        }
        query
    }
}

fn push_opt(query: &mut Vec<(&'static str, String)>, name: &'static str, value: &Option<String>) {
    if let Some(value) = value.as_ref().filter(|value| !value.trim().is_empty()) {
        query.push((name, value.clone()));
    }
}

fn http_error(status: StatusCode, body: String, retry_after: Option<u64>) -> CliError {
    let details = serde_json::from_str::<Value>(&body)
        .ok()
        .or_else(|| (!body.trim().is_empty()).then(|| serde_json::json!({ "body": body })));

    let error = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            CliError::auth("Granola API authentication failed")
        }
        StatusCode::NOT_FOUND => CliError::not_found("Granola API resource not found"),
        StatusCode::TOO_MANY_REQUESTS => {
            CliError::rate_limited("Granola API rate limit exceeded").with_retry_after(retry_after)
        }
        status if status.is_client_error() => {
            CliError::invalid_input(format!("Granola API request failed with status {status}"))
        }
        status => CliError::general(format!("Granola API request failed with status {status}")),
    };

    if let Some(details) = details {
        error.with_details(details)
    } else {
        error
    }
}

pub fn validate_page_size(page_size: u8) -> Result<u8, CliError> {
    if (1..=30).contains(&page_size) {
        Ok(page_size)
    } else {
        Err(CliError::invalid_input(
            "page size must be between 1 and 30 for the Granola API",
        ))
    }
}

pub fn resolve_api_key(api_key_override: Option<String>) -> Result<String, CliError> {
    if let Some(api_key) = api_key_override.filter(|key| !key.trim().is_empty()) {
        return Ok(api_key);
    }

    crate::keyring::get_key()?.ok_or_else(|| {
        CliError::auth(
            "no Granola API key configured; run `granola auth login` or pass `--api-key`",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn validates_granola_page_size_bounds() {
        assert_eq!(validate_page_size(1).unwrap(), 1);
        assert_eq!(validate_page_size(30).unwrap(), 30);
        assert!(validate_page_size(31).is_err());
    }

    #[tokio::test]
    async fn list_notes_sends_auth_and_query_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/notes"))
            .and(header("authorization", "Bearer test-key"))
            .and(query_param("page_size", "2"))
            .and(query_param("created_after", "2026-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                include_str!("../tests/fixtures/list_notes.json"),
                "application/json",
            ))
            .mount(&server)
            .await;

        let client = GranolaClient::with_base_url(server.uri(), "test-key".to_string()).unwrap();
        let response = client
            .list_notes(&ListNotesParams {
                created_after: Some("2026-06-01".to_string()),
                page_size: Some(2),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(response.notes.len(), 1);
        assert_eq!(response.notes[0].id, "not_AAAAAAAAAAAAAA");
    }

    #[tokio::test]
    async fn get_note_deserializes_success_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/notes/not_AAAAAAAAAAAAAA"))
            .and(query_param("include", "transcript"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                include_str!("../tests/fixtures/get_note_with_transcript.json"),
                "application/json",
            ))
            .mount(&server)
            .await;

        let client = GranolaClient::with_base_url(server.uri(), "test-key".to_string()).unwrap();
        let note = client.get_note("not_AAAAAAAAAAAAAA", true).await.unwrap();

        assert_eq!(note.id, "not_BBBBBBBBBBBBBB");
        assert_eq!(note.transcript.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn list_folders_deserializes_success_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/folders"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                include_str!("../tests/fixtures/list_folders.json"),
                "application/json",
            ))
            .mount(&server)
            .await;

        let client = GranolaClient::with_base_url(server.uri(), "test-key".to_string()).unwrap();
        let response = client
            .list_folders(&ListFoldersParams::default())
            .await
            .unwrap();

        assert_eq!(response.folders.len(), 2);
    }

    #[tokio::test]
    async fn maps_auth_errors() {
        let err = error_for_status(StatusCode::UNAUTHORIZED, None).await;
        assert_eq!(err.kind, ErrorKind::Auth);
        assert_eq!(err.code(), 3);
    }

    #[tokio::test]
    async fn maps_not_found_errors() {
        let err = error_for_status(StatusCode::NOT_FOUND, None).await;
        assert_eq!(err.kind, ErrorKind::NotFound);
        assert_eq!(err.code(), 2);
    }

    #[tokio::test]
    async fn maps_rate_limit_errors_with_retry_after() {
        let err = error_for_status(StatusCode::TOO_MANY_REQUESTS, Some("7")).await;
        assert_eq!(err.kind, ErrorKind::RateLimited);
        assert_eq!(err.code(), 4);
        assert_eq!(err.retry_after, Some(7));
    }

    #[tokio::test]
    async fn malformed_json_is_a_general_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/folders"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
            .mount(&server)
            .await;

        let client = GranolaClient::with_base_url(server.uri(), "test-key".to_string()).unwrap();
        let err = client
            .list_folders(&ListFoldersParams::default())
            .await
            .unwrap_err();

        assert_eq!(err.kind, ErrorKind::General);
    }

    async fn error_for_status(status: StatusCode, retry_after: Option<&str>) -> CliError {
        let server = MockServer::start().await;
        let mut response = ResponseTemplate::new(status.as_u16())
            .set_body_json(serde_json::json!({ "message": "redacted error" }));
        if let Some(retry_after) = retry_after {
            response = response.append_header("retry-after", retry_after);
        }
        Mock::given(method("GET"))
            .and(path("/v1/folders"))
            .respond_with(response)
            .mount(&server)
            .await;

        let client = GranolaClient::with_base_url(server.uri(), "test-key".to_string()).unwrap();
        client
            .list_folders(&ListFoldersParams::default())
            .await
            .unwrap_err()
    }
}
