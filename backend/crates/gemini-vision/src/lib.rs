use std::{fs, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use reqwest::{StatusCode, Url, blocking::Client};
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub endpoint: String,
    pub api_key: String,
    pub timeout: Duration,
}
pub struct GeminiVisionClient {
    endpoint: Url,
    api_key: String,
    client: Client,
}

impl GeminiVisionClient {
    pub fn new(config: ClientConfig) -> Result<Self, GeminiVisionError> {
        let endpoint = validate_base_url(&config.endpoint)?;
        if config.timeout.is_zero() {
            return Err(GeminiVisionError::InvalidTimeout);
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
    ) -> Result<String, GeminiVisionError> {
        if model.trim().is_empty() {
            return Err(GeminiVisionError::EmptyModel);
        }
        if instruction.trim().is_empty() {
            return Err(GeminiVisionError::EmptyInstruction);
        }
        let endpoint = endpoint_url(&self.endpoint, model);
        let bytes = fs::read(image_path).map_err(|error| GeminiVisionError::ImageRead {
            path: image_path.to_string(),
            message: error.to_string(),
        })?;
        let mime = image_mime(&bytes)
            .ok_or_else(|| GeminiVisionError::UnsupportedImage(image_path.to_string()))?;
        let request = json!({ "contents": [{ "role": "user", "parts": [{ "text": instruction }, { "inlineData": { "mimeType": mime, "data": BASE64.encode(bytes) } }] }], "generationConfig": { "temperature": temperature, "maxOutputTokens": max_tokens } });
        let mut builder = self.client.post(endpoint).json(&request);
        if !self.api_key.trim().is_empty() {
            builder = builder.header("x-goog-api-key", self.api_key.trim());
        }
        let response = builder.send()?;
        let status = response.status();
        let body = response.text()?;
        if !status.is_success() {
            return Err(GeminiVisionError::Api {
                status,
                message: body.chars().take(2_000).collect(),
            });
        }
        let value: Value = serde_json::from_str(&body)?;
        let text = value
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|x| x.first())
            .and_then(|x| x.get("content"))
            .and_then(|x| x.get("parts"))
            .and_then(Value::as_array)
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|part| part.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("")
            });
        let text = text
            .filter(|text| !text.trim().is_empty())
            .ok_or(GeminiVisionError::EmptyResponse)?;
        Ok(text)
    }
}

fn validate_base_url(value: &str) -> Result<Url, GeminiVisionError> {
    let mut url = Url::parse(value.trim())
        .map_err(|error| GeminiVisionError::InvalidUrl(error.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(GeminiVisionError::InvalidEndpoint(value.trim().to_string()));
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}
fn endpoint_url(base: &Url, model: &str) -> Url {
    let mut url = base.clone();
    let path = url.path().trim_end_matches('/');
    let model = model.trim().strip_prefix("models/").unwrap_or(model.trim());
    if path.contains(":generateContent") {
        return url;
    }
    let prefix = if path.ends_with("/v1beta") {
        path.to_string()
    } else {
        format!("{path}/v1beta")
    };
    url.set_path(&format!("{prefix}/models/{model}:generateContent"));
    url
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

#[derive(Debug, Error)]
pub enum GeminiVisionError {
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
    #[error("Gemini vision API returned HTTP {status}: {message}")]
    Api { status: StatusCode, message: String },
    #[error("Gemini vision API returned no text")]
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
            endpoint_url(
                &validate_base_url("https://generativelanguage.googleapis.com").unwrap(),
                "gemini-2.0-flash"
            )
            .path(),
            "/v1beta/models/gemini-2.0-flash:generateContent"
        );
    }
}
