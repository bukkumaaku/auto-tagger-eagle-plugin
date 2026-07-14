use std::time::Duration;

use reqwest::{StatusCode, Url, blocking::Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_ERROR_BODY_LENGTH: usize = 2_000;

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub endpoint: String,
    pub api_key: String,
    pub timeout: Duration,
}

impl ClientConfig {
    pub fn new(endpoint: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct EmbeddingData {
    pub index: usize,
    pub embedding: Vec<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct EmbeddingUsage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct EmbeddingResponse {
    #[serde(default)]
    pub model: String,
    pub data: Vec<EmbeddingData>,
    #[serde(default)]
    pub usage: EmbeddingUsage,
}

pub struct OpenAiEmbeddingClient {
    endpoint: Url,
    api_key: String,
    client: Client,
}

impl OpenAiEmbeddingClient {
    pub fn new(config: ClientConfig) -> Result<Self, OpenAiEmbeddingError> {
        let mut endpoint = Url::parse(config.endpoint.trim())
            .map_err(|error| OpenAiEmbeddingError::InvalidUrl(error.to_string()))?;
        if !matches!(endpoint.scheme(), "http" | "https") {
            return Err(OpenAiEmbeddingError::UnsupportedScheme(
                endpoint.scheme().to_string(),
            ));
        }
        if endpoint.cannot_be_a_base() || endpoint.host_str().is_none() {
            return Err(OpenAiEmbeddingError::InvalidEndpoint(
                config.endpoint.trim().to_string(),
            ));
        }
        if config.timeout.is_zero() {
            return Err(OpenAiEmbeddingError::InvalidTimeout);
        }
        if !endpoint.path().trim_matches('/').ends_with("embeddings") {
            let prefix = endpoint.path().trim_end_matches('/');
            endpoint.set_path(&format!("{prefix}/v1/embeddings"));
        }
        endpoint.set_query(None);
        endpoint.set_fragment(None);

        let client = Client::builder().timeout(config.timeout).build()?;
        Ok(Self {
            endpoint,
            api_key: config.api_key,
            client,
        })
    }

    pub fn embed(
        &self,
        request: &EmbeddingRequest,
    ) -> Result<EmbeddingResponse, OpenAiEmbeddingError> {
        if request.model.trim().is_empty() {
            return Err(OpenAiEmbeddingError::EmptyModel);
        }
        if request.input.is_null() {
            return Err(OpenAiEmbeddingError::EmptyInput);
        }
        if request.dimensions == Some(0) {
            return Err(OpenAiEmbeddingError::InvalidDimensions);
        }

        let mut builder = self.client.post(self.endpoint.clone()).json(request);
        if !self.api_key.trim().is_empty() {
            builder = builder.bearer_auth(self.api_key.trim());
        }

        let response = builder.send()?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(OpenAiEmbeddingError::Api {
                status,
                message: api_error_message(&body),
            });
        }

        let result = response.json::<EmbeddingResponse>()?;
        validate_response(&result)?;
        Ok(result)
    }
}

fn validate_response(response: &EmbeddingResponse) -> Result<(), OpenAiEmbeddingError> {
    if response.data.is_empty() {
        return Err(OpenAiEmbeddingError::EmptyResponse);
    }

    let dimension = response.data[0].embedding.len();
    if dimension == 0 {
        return Err(OpenAiEmbeddingError::EmptyVector { index: 0 });
    }
    for item in &response.data {
        if item.embedding.is_empty() {
            return Err(OpenAiEmbeddingError::EmptyVector { index: item.index });
        }
        if item.embedding.len() != dimension {
            return Err(OpenAiEmbeddingError::DimensionMismatch {
                index: item.index,
                expected: dimension,
                actual: item.embedding.len(),
            });
        }
        if item.embedding.iter().any(|value| !value.is_finite()) {
            return Err(OpenAiEmbeddingError::NonFiniteVector { index: item.index });
        }
    }
    Ok(())
}

fn api_error_message(body: &str) -> String {
    let parsed = serde_json::from_str::<ApiErrorEnvelope>(body)
        .ok()
        .and_then(|envelope| envelope.error.message)
        .filter(|message| !message.trim().is_empty());
    truncate(parsed.as_deref().unwrap_or(body.trim()))
}

fn truncate(value: &str) -> String {
    let mut chars = value.chars();
    let truncated = chars
        .by_ref()
        .take(MAX_ERROR_BODY_LENGTH)
        .collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else if truncated.is_empty() {
        "request failed without an error message".to_string()
    } else {
        truncated
    }
}

#[derive(Debug, Deserialize)]
struct ApiErrorEnvelope {
    error: ApiError,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Error)]
pub enum OpenAiEmbeddingError {
    #[error("invalid endpoint URL: {0}")]
    InvalidUrl(String),
    #[error("endpoint uses unsupported URL scheme `{0}`")]
    UnsupportedScheme(String),
    #[error("invalid endpoint URL `{0}`")]
    InvalidEndpoint(String),
    #[error("request timeout must be greater than zero")]
    InvalidTimeout,
    #[error("model cannot be empty")]
    EmptyModel,
    #[error("input cannot be null")]
    EmptyInput,
    #[error("dimensions must be greater than zero")]
    InvalidDimensions,
    #[error("failed to call OpenAI-compatible embeddings endpoint: {0}")]
    Http(#[from] reqwest::Error),
    #[error("embedding API returned HTTP {status}: {message}")]
    Api { status: StatusCode, message: String },
    #[error("embedding API returned no vectors")]
    EmptyResponse,
    #[error("embedding at index {index} is empty")]
    EmptyVector { index: usize },
    #[error("embedding at index {index} has dimension {actual}, but expected {expected}")]
    DimensionMismatch {
        index: usize,
        expected: usize,
        actual: usize,
    },
    #[error("embedding at index {index} contains a non-finite value")]
    NonFiniteVector { index: usize },
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use serde_json::json;

    use super::*;

    #[test]
    fn sends_openai_compatible_request_and_parses_response() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 8_192];
            let length = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..length]);
            assert!(request.starts_with("POST /v1/embeddings HTTP/1.1"));
            assert!(request.contains("authorization: Bearer secret"));
            assert!(request.contains("\"model\":\"gemini-embedding-2-preview\""));
            assert!(request.contains("\"input\":\"hello\""));
            assert!(request.contains("\"dimensions\":3"));

            let body = r#"{"model":"gemini-embedding-2-preview","data":[{"index":0,"embedding":[0.1,0.2,0.3]}],"usage":{"prompt_tokens":1,"total_tokens":1}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });

        let client = OpenAiEmbeddingClient::new(ClientConfig::new(
            format!("http://{address}/v1/embeddings"),
            "secret",
        ))
        .unwrap();
        let response = client
            .embed(&EmbeddingRequest {
                model: "gemini-embedding-2-preview".to_string(),
                input: json!("hello"),
                dimensions: Some(3),
            })
            .unwrap();

        server.join().unwrap();
        assert_eq!(response.data[0].embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(response.usage.total_tokens, 1);
    }

    #[test]
    fn completes_a_base_url_with_the_embeddings_path() {
        let client =
            OpenAiEmbeddingClient::new(ClientConfig::new("https://example.com", "secret")).unwrap();

        assert_eq!(
            client.endpoint.as_str(),
            "https://example.com/v1/embeddings"
        );
    }

    #[test]
    fn reports_openai_error_message_without_exposing_the_key() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 4_096];
            let _ = stream.read(&mut buffer).unwrap();
            let body = r#"{"error":{"message":"unknown model"}}"#;
            write!(
                stream,
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });

        let client = OpenAiEmbeddingClient::new(ClientConfig::new(
            format!("http://{address}/v1/embeddings"),
            "do-not-leak",
        ))
        .unwrap();
        let error = client
            .embed(&EmbeddingRequest {
                model: "missing".to_string(),
                input: json!("hello"),
                dimensions: None,
            })
            .unwrap_err();

        server.join().unwrap();
        let message = error.to_string();
        assert!(message.contains("unknown model"));
        assert!(!message.contains("do-not-leak"));
    }

    #[test]
    fn rejects_invalid_response_vectors() {
        let response = EmbeddingResponse {
            model: "model".to_string(),
            data: vec![
                EmbeddingData {
                    index: 0,
                    embedding: vec![1.0, 2.0],
                },
                EmbeddingData {
                    index: 1,
                    embedding: vec![3.0],
                },
            ],
            usage: EmbeddingUsage::default(),
        };

        assert!(matches!(
            validate_response(&response),
            Err(OpenAiEmbeddingError::DimensionMismatch { index: 1, .. })
        ));
    }
}
