use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use image::{DynamicImage, codecs::jpeg::JpegEncoder, imageops::FilterType};
use reqwest::{StatusCode, Url, blocking::Client};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_ERROR_BODY_LENGTH: usize = 2_000;
const MAX_REQUEST_ATTEMPTS: usize = 4;
const BASE_RETRY_DELAY: Duration = Duration::from_millis(500);
pub const MIN_DIMENSION: usize = 128;
pub const MAX_DIMENSION: usize = 3_072;
const MAX_ENCODED_IMAGE_BYTES: usize = 700 * 1_024;
const IMAGE_DIMENSION_STEPS: [u32; 4] = [1_024, 768, 512, 384];
const JPEG_QUALITY_STEPS: [u8; 4] = [85, 75, 65, 50];

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub base_url: String,
    pub api_key: String,
    pub timeout: Duration,
}

impl ClientConfig {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

pub struct GeminiEmbeddingClient {
    base_url: Url,
    api_key: String,
    client: Client,
}

impl GeminiEmbeddingClient {
    pub fn new(config: ClientConfig) -> Result<Self, GeminiEmbeddingError> {
        let base_url = validate_base_url(&config.base_url)?;
        if config.timeout.is_zero() {
            return Err(GeminiEmbeddingError::InvalidTimeout);
        }
        Ok(Self {
            base_url,
            api_key: config.api_key,
            client: Client::builder().timeout(config.timeout).build()?,
        })
    }

    pub fn embed_text(
        &self,
        model: &str,
        text: &str,
        dimension: usize,
    ) -> Result<Vec<f32>, GeminiEmbeddingError> {
        if text.trim().is_empty() {
            return Err(GeminiEmbeddingError::EmptyText);
        }
        self.embed(
            model,
            dimension,
            Part::Text {
                text: text.to_string(),
            },
        )
    }

    pub fn embed_image(
        &self,
        model: &str,
        mime_type: &str,
        base64_data: &str,
        dimension: usize,
    ) -> Result<Vec<f32>, GeminiEmbeddingError> {
        if mime_type.trim().is_empty() || base64_data.is_empty() {
            return Err(GeminiEmbeddingError::EmptyImage);
        }
        self.embed(
            model,
            dimension,
            Part::InlineData {
                inline_data: InlineData {
                    mime_type: mime_type.to_string(),
                    data: base64_data.to_string(),
                },
            },
        )
    }

    pub fn embed_image_bytes(
        &self,
        model: &str,
        image_bytes: &[u8],
        dimension: usize,
    ) -> Result<Vec<f32>, GeminiEmbeddingError> {
        let jpeg = prepare_image_bytes(image_bytes)?;
        self.embed_prepared_image(model, &jpeg, dimension)
    }

    pub fn embed_prepared_image(
        &self,
        model: &str,
        jpeg_bytes: &[u8],
        dimension: usize,
    ) -> Result<Vec<f32>, GeminiEmbeddingError> {
        if jpeg_bytes.is_empty() {
            return Err(GeminiEmbeddingError::EmptyImage);
        }
        self.embed_image(model, "image/jpeg", &BASE64.encode(jpeg_bytes), dimension)
    }

    fn embed(
        &self,
        model: &str,
        dimension: usize,
        part: Part,
    ) -> Result<Vec<f32>, GeminiEmbeddingError> {
        let model = validate_model(model)?;
        validate_dimension(dimension)?;
        let endpoint = endpoint_url(&self.base_url, model);
        let request = EmbedContentRequest {
            content: Content { parts: vec![part] },
            output_dimensionality: dimension,
        };
        for attempt in 0..MAX_REQUEST_ATTEMPTS {
            let mut builder = self.client.post(endpoint.clone()).json(&request);
            if !self.api_key.trim().is_empty() {
                builder = builder.header("x-goog-api-key", self.api_key.trim());
            }
            let response = match builder.send() {
                Ok(response) => response,
                Err(error)
                    if attempt + 1 < MAX_REQUEST_ATTEMPTS
                        && (error.is_connect() || error.is_timeout()) =>
                {
                    std::thread::sleep(retry_delay(attempt, None));
                    continue;
                }
                Err(error) => return Err(GeminiEmbeddingError::Http(error)),
            };
            let status = response.status();
            if status.is_success() {
                let response = response.json::<EmbedContentResponse>()?;
                return validate_embedding(response.embedding.values, dimension);
            }
            if attempt + 1 < MAX_REQUEST_ATTEMPTS && retryable_status(status) {
                let retry_after = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(Duration::from_secs);
                std::thread::sleep(retry_delay(attempt, retry_after));
                continue;
            }
            let body = response.text().unwrap_or_default();
            return Err(GeminiEmbeddingError::Api {
                status,
                message: api_error_message(&body),
            });
        }
        unreachable!("Gemini request loop always returns")
    }
}

pub fn prepare_image_bytes(image_bytes: &[u8]) -> Result<Vec<u8>, GeminiEmbeddingError> {
    if image_bytes.is_empty() {
        return Err(GeminiEmbeddingError::EmptyImage);
    }
    let image = image::load_from_memory(image_bytes)
        .map_err(|error| GeminiEmbeddingError::InvalidImage(error.to_string()))?;
    encode_image_for_request(&image)
}

fn encode_image_for_request(image: &DynamicImage) -> Result<Vec<u8>, GeminiEmbeddingError> {
    for max_dimension in IMAGE_DIMENSION_STEPS {
        let image = DynamicImage::ImageRgb8(resize_image(image, max_dimension).to_rgb8());
        for quality in JPEG_QUALITY_STEPS {
            let mut jpeg = Vec::new();
            JpegEncoder::new_with_quality(&mut jpeg, quality)
                .encode_image(&image)
                .map_err(|error| GeminiEmbeddingError::InvalidImage(error.to_string()))?;
            if jpeg.len() <= MAX_ENCODED_IMAGE_BYTES {
                return Ok(jpeg);
            }
        }
    }
    Err(GeminiEmbeddingError::ImageTooLarge)
}

fn retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn retry_delay(attempt: usize, retry_after: Option<Duration>) -> Duration {
    retry_after.unwrap_or_else(|| BASE_RETRY_DELAY.saturating_mul(1 << attempt))
}

fn resize_image(image: &DynamicImage, max_dimension: u32) -> DynamicImage {
    if image.width() <= max_dimension && image.height() <= max_dimension {
        image.clone()
    } else {
        image.resize(max_dimension, max_dimension, FilterType::Triangle)
    }
}

fn validate_base_url(value: &str) -> Result<Url, GeminiEmbeddingError> {
    let mut url = Url::parse(value.trim())
        .map_err(|error| GeminiEmbeddingError::InvalidUrl(error.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(GeminiEmbeddingError::UnsupportedScheme(
            url.scheme().to_string(),
        ));
    }
    if url.cannot_be_a_base() || url.host_str().is_none() {
        return Err(GeminiEmbeddingError::InvalidEndpoint(
            value.trim().to_string(),
        ));
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn validate_model(model: &str) -> Result<&str, GeminiEmbeddingError> {
    let model = model.trim().strip_prefix("models/").unwrap_or(model.trim());
    if model.is_empty() {
        return Err(GeminiEmbeddingError::EmptyModel);
    }
    if !model
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(GeminiEmbeddingError::InvalidModel(model.to_string()));
    }
    Ok(model)
}

fn validate_dimension(dimension: usize) -> Result<(), GeminiEmbeddingError> {
    if (MIN_DIMENSION..=MAX_DIMENSION).contains(&dimension) {
        Ok(())
    } else {
        Err(GeminiEmbeddingError::InvalidDimension(dimension))
    }
}

fn endpoint_url(base_url: &Url, model: &str) -> Url {
    let mut endpoint = base_url.clone();
    endpoint.set_path(&format!("/v1beta/models/{model}:embedContent"));
    endpoint.set_query(None);
    endpoint.set_fragment(None);
    endpoint
}

fn validate_embedding(
    embedding: Vec<f32>,
    expected_dimension: usize,
) -> Result<Vec<f32>, GeminiEmbeddingError> {
    if embedding.len() != expected_dimension {
        return Err(GeminiEmbeddingError::DimensionMismatch {
            expected: expected_dimension,
            actual: embedding.len(),
        });
    }
    if embedding.iter().any(|value| !value.is_finite()) {
        return Err(GeminiEmbeddingError::NonFiniteVector);
    }
    Ok(embedding)
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

#[derive(Debug, Serialize)]
struct EmbedContentRequest {
    content: Content,
    output_dimensionality: usize,
}

#[derive(Debug, Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum Part {
    Text { text: String },
    InlineData { inline_data: InlineData },
}

#[derive(Debug, Serialize)]
struct InlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug, Deserialize)]
struct EmbedContentResponse {
    embedding: ContentEmbedding,
}

#[derive(Debug, Deserialize)]
struct ContentEmbedding {
    values: Vec<f32>,
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
pub enum GeminiEmbeddingError {
    #[error("invalid Gemini base URL: {0}")]
    InvalidUrl(String),
    #[error("Gemini base URL uses unsupported scheme `{0}`")]
    UnsupportedScheme(String),
    #[error("invalid Gemini base URL `{0}`")]
    InvalidEndpoint(String),
    #[error("request timeout must be greater than zero")]
    InvalidTimeout,
    #[error("Gemini model cannot be empty")]
    EmptyModel,
    #[error("invalid Gemini model name `{0}`")]
    InvalidModel(String),
    #[error("embedding text cannot be empty")]
    EmptyText,
    #[error("embedding image cannot be empty")]
    EmptyImage,
    #[error("failed to decode or encode embedding image: {0}")]
    InvalidImage(String),
    #[error("embedding image remains too large after compression")]
    ImageTooLarge,
    #[error(
        "Gemini embedding dimension must be between {MIN_DIMENSION} and {MAX_DIMENSION}, got {0}"
    )]
    InvalidDimension(usize),
    #[error("failed to call Gemini embeddings endpoint: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Gemini embedding API returned HTTP {status}: {message}")]
    Api { status: StatusCode, message: String },
    #[error("Gemini embedding has dimension {actual}, expected {expected}")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error("Gemini embedding contains a non-finite value")]
    NonFiniteVector,
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use super::*;

    fn read_http_request(stream: &mut impl Read) -> Vec<u8> {
        let mut request = Vec::new();
        let mut expected_length = None;
        loop {
            let mut buffer = [0_u8; 8_192];
            let length = stream.read(&mut buffer).unwrap();
            assert!(
                length > 0,
                "connection closed before the request was complete"
            );
            request.extend_from_slice(&buffer[..length]);

            if expected_length.is_none()
                && let Some(header_end) =
                    request.windows(4).position(|window| window == b"\r\n\r\n")
            {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                expected_length = Some(header_end + 4 + content_length);
            }

            if expected_length.is_some_and(|expected| request.len() >= expected) {
                return request;
            }
        }
    }

    #[test]
    fn sends_native_text_request_and_completes_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 8_192];
            let length = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..length]);
            assert!(request.starts_with(
                "POST /v1beta/models/gemini-embedding-2-preview:embedContent HTTP/1.1"
            ));
            assert!(request.contains("x-goog-api-key: secret"));
            assert!(request.contains("\"text\":\"hello\""));
            assert!(request.contains("\"output_dimensionality\":128"));
            let values = std::iter::repeat_n("0.1", 128)
                .collect::<Vec<_>>()
                .join(",");
            let body = format!(r#"{{"embedding":{{"values":[{values}]}}}}"#);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            )
            .unwrap();
        });

        let client =
            GeminiEmbeddingClient::new(ClientConfig::new(format!("http://{address}"), "secret"))
                .unwrap();
        let embedding = client
            .embed_text("gemini-embedding-2-preview", "hello", 128)
            .unwrap();
        server.join().unwrap();
        assert_eq!(embedding.len(), 128);
    }

    #[test]
    fn retries_rate_limited_requests() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut first, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 8_192];
            let first_length = first.read(&mut buffer).unwrap();
            assert!(first_length > 0);
            write!(
                first,
                "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .unwrap();

            let (mut second, _) = listener.accept().unwrap();
            let second_length = second.read(&mut buffer).unwrap();
            assert!(second_length > 0);
            let values = std::iter::repeat_n("0.1", 128)
                .collect::<Vec<_>>()
                .join(",");
            let body = format!(r#"{{"embedding":{{"values":[{values}]}}}}"#);
            write!(
                second,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });

        let client =
            GeminiEmbeddingClient::new(ClientConfig::new(format!("http://{address}"), "secret"))
                .unwrap();
        let embedding = client
            .embed_text("gemini-embedding-2-preview", "retry me", 128)
            .unwrap();
        server.join().unwrap();
        assert_eq!(embedding.len(), 128);
    }

    #[test]
    fn sends_compressed_native_image_request() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_http_request(&mut stream);
            let request = String::from_utf8_lossy(&request);
            assert!(request.contains("\"inline_data\""));
            assert!(request.contains("\"mime_type\":\"image/jpeg\""));
            assert!(!request.contains("\"data\":\"YWJj\""));
            assert!(request.len() < 1_048_576);
            let values = std::iter::repeat_n("0.1", 128)
                .collect::<Vec<_>>()
                .join(",");
            let body = format!(r#"{{"embedding":{{"values":[{values}]}}}}"#);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            )
            .unwrap();
        });

        let client =
            GeminiEmbeddingClient::new(ClientConfig::new(format!("http://{address}"), "secret"))
                .unwrap();
        let image = DynamicImage::new_rgb8(2_048, 1_024);
        let mut png = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let embedding = client
            .embed_image_bytes("gemini-embedding-2-preview", &png, 128)
            .unwrap();
        server.join().unwrap();
        assert_eq!(embedding.len(), 128);
    }

    #[test]
    fn keeps_encoded_image_below_proxy_limit() {
        let image = DynamicImage::ImageRgb8(image::RgbImage::from_fn(2_048, 2_048, |x, y| {
            image::Rgb([
                (x.wrapping_mul(31) ^ y.wrapping_mul(17)) as u8,
                (x.wrapping_mul(13) ^ y.wrapping_mul(29)) as u8,
                (x.wrapping_mul(7) ^ y.wrapping_mul(37)) as u8,
            ])
        }));
        let jpeg = encode_image_for_request(&image).unwrap();
        assert!(jpeg.len() <= MAX_ENCODED_IMAGE_BYTES);
        let decoded = image::load_from_memory(&jpeg).unwrap();
        assert!(decoded.width() <= IMAGE_DIMENSION_STEPS[0]);
        assert!(decoded.height() <= IMAGE_DIMENSION_STEPS[0]);
    }

    #[test]
    fn rejects_dimensions_outside_the_official_range() {
        assert!(matches!(
            validate_dimension(127),
            Err(GeminiEmbeddingError::InvalidDimension(127))
        ));
        assert!(validate_dimension(768).is_ok());
        assert!(validate_dimension(1_536).is_ok());
        assert!(validate_dimension(3_072).is_ok());
        assert!(matches!(
            validate_dimension(3_073),
            Err(GeminiEmbeddingError::InvalidDimension(3_073))
        ));
    }
}
