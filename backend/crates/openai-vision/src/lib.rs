use std::{fs, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use reqwest::{StatusCode, Url, blocking::Client};
use serde_json::{Value, json};
use thiserror::Error;

const MAX_ERROR_BODY_LENGTH: usize = 2_000;

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub endpoint: String,
    pub api_key: String,
    pub timeout: Duration,
}

pub struct OpenAiVisionClient {
    endpoint: Url,
    api_key: String,
    client: Client,
}

impl OpenAiVisionClient {
    pub fn new(config: ClientConfig) -> Result<Self, OpenAiVisionError> {
        let endpoint = complete_endpoint(&config.endpoint)?;
        if config.timeout.is_zero() {
            return Err(OpenAiVisionError::InvalidTimeout);
        }
        Ok(Self {
            endpoint,
            api_key: config.api_key,
            client: Client::builder().timeout(config.timeout).build()?,
        })
    }

    pub fn process_image(
        &self,
        model: &str,
        image_path: &str,
        instruction: &str,
        temperature: f32,
        max_tokens: usize,
    ) -> Result<String, OpenAiVisionError> {
        if model.trim().is_empty() {
            return Err(OpenAiVisionError::EmptyModel);
        }
        if instruction.trim().is_empty() {
            return Err(OpenAiVisionError::EmptyInstruction);
        }
        let bytes = fs::read(image_path).map_err(|error| OpenAiVisionError::ImageRead {
            path: image_path.to_string(),
            message: error.to_string(),
        })?;
        let mime = image_mime(&bytes)
            .ok_or_else(|| OpenAiVisionError::UnsupportedImage(image_path.to_string()))?;
        let data_url = format!("data:{mime};base64,{}", BASE64.encode(bytes));
        let request = json!({ "model": model.trim(), "messages": [{ "role": "user", "content": [{ "type": "text", "text": instruction }, { "type": "image_url", "image_url": { "url": data_url } }] }], "temperature": temperature, "max_tokens": max_tokens });
        let mut builder = self.client.post(self.endpoint.clone()).json(&request);
        if !self.api_key.trim().is_empty() {
            builder = builder.bearer_auth(self.api_key.trim());
        }
        let response = builder.send()?;
        let status = response.status();
        let body = response.text()?;
        if !status.is_success() {
            return Err(OpenAiVisionError::Api {
                status,
                message: truncate(&body),
            });
        }
        let value: Value = serde_json::from_str(&body)?;
        extract_content(&value).ok_or(OpenAiVisionError::EmptyResponse)
    }
}

fn complete_endpoint(value: &str) -> Result<Url, OpenAiVisionError> {
    let mut url = Url::parse(value.trim())
        .map_err(|error| OpenAiVisionError::InvalidUrl(error.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(OpenAiVisionError::InvalidEndpoint(value.trim().to_string()));
    }
    let path = url.path().trim_end_matches('/');
    let path = if path.ends_with("/chat/completions") {
        path.to_string()
    } else if path.ends_with("/v1") {
        format!("{path}/chat/completions")
    } else {
        format!("{path}/v1/chat/completions")
    };
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn extract_content(value: &Value) -> Option<String> {
    let content = value
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?;
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }
    content
        .as_array()?
        .iter()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .map(str::to_string)
        .reduce(|a, b| format!("{a}{b}"))
}
fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else {
        None
    }
}
fn truncate(value: &str) -> String {
    value.chars().take(MAX_ERROR_BODY_LENGTH).collect()
}

#[derive(Debug, Error)]
pub enum OpenAiVisionError {
    #[error("invalid endpoint URL: {0}")]
    InvalidUrl(String),
    #[error("invalid endpoint URL `{0}`")]
    InvalidEndpoint(String),
    #[error("request timeout must be greater than zero")]
    InvalidTimeout,
    #[error("model cannot be empty")]
    EmptyModel,
    #[error("instruction cannot be empty")]
    EmptyInstruction,
    #[error("failed to read image {path}: {message}")]
    ImageRead { path: String, message: String },
    #[error("unsupported image format: {0}")]
    UnsupportedImage(String),
    #[error("OpenAI vision API returned HTTP {status}: {message}")]
    Api { status: StatusCode, message: String },
    #[error("OpenAI vision API returned no text")]
    EmptyResponse,
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("invalid API response: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completes_base_urls() {
        assert_eq!(
            complete_endpoint("https://example.com").unwrap().as_str(),
            "https://example.com/v1/chat/completions"
        );
        assert_eq!(
            complete_endpoint("https://example.com/v1")
                .unwrap()
                .as_str(),
            "https://example.com/v1/chat/completions"
        );
    }
}
