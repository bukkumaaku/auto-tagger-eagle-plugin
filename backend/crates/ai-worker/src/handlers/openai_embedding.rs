use std::time::Duration;

use openai_embedding::{ClientConfig, EmbeddingRequest, OpenAiEmbeddingClient};
use protocol::{
    OpenAiEmbeddingRequest, OpenAiEmbeddingResult, OpenAiEmbeddingUsage, OpenAiEmbeddingVector,
};

use super::{HandlerError, HandlerResult};

const DEFAULT_TIMEOUT_MILLISECONDS: u64 = 120_000;
const MAX_TIMEOUT_MILLISECONDS: u64 = 30 * 60 * 1_000;

pub fn handle(request: OpenAiEmbeddingRequest) -> HandlerResult<OpenAiEmbeddingResult> {
    let timeout_milliseconds = request
        .timeout_milliseconds
        .unwrap_or(DEFAULT_TIMEOUT_MILLISECONDS);
    if !(1..=MAX_TIMEOUT_MILLISECONDS).contains(&timeout_milliseconds) {
        return Err(HandlerError::new(
            "OPENAI_EMBEDDING_INVALID_TIMEOUT",
            format!("timeoutMilliseconds must be between 1 and {MAX_TIMEOUT_MILLISECONDS}"),
        ));
    }

    let mut config = ClientConfig::new(request.endpoint, request.api_key);
    config.timeout = Duration::from_millis(timeout_milliseconds);
    let client = OpenAiEmbeddingClient::new(config).map_err(client_error)?;
    let response = client
        .embed(&EmbeddingRequest {
            model: request.model,
            input: request.input,
            dimensions: request.dimensions,
        })
        .map_err(client_error)?;
    let dimensions = response.data[0].embedding.len();

    Ok(OpenAiEmbeddingResult {
        model: response.model,
        dimensions,
        data: response
            .data
            .into_iter()
            .map(|item| OpenAiEmbeddingVector {
                index: item.index,
                embedding: item.embedding,
            })
            .collect(),
        usage: OpenAiEmbeddingUsage {
            prompt_tokens: response.usage.prompt_tokens,
            total_tokens: response.usage.total_tokens,
        },
    })
}

fn client_error(error: impl std::fmt::Display) -> HandlerError {
    HandlerError::new("OPENAI_EMBEDDING_FAILED", error.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn rejects_invalid_timeout_before_calling_endpoint() {
        let error = handle(OpenAiEmbeddingRequest {
            endpoint: "https://example.com/v1/embeddings".to_string(),
            api_key: String::new(),
            model: "model".to_string(),
            input: json!("hello"),
            dimensions: None,
            timeout_milliseconds: Some(0),
        })
        .unwrap_err();

        assert_eq!(error.code, "OPENAI_EMBEDDING_INVALID_TIMEOUT");
    }
}
