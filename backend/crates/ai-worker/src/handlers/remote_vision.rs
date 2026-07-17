use super::{HandlerError, HandlerResult};
use gemini_vision::{ClientConfig as GeminiConfig, GeminiVisionClient};
use openai_vision::{ClientConfig as OpenAiConfig, OpenAiVisionClient};
use protocol::{
    RemoteVisionProcessImageRequest, RemoteVisionProcessImageResult, RemoteVisionProvider,
};
use std::time::Duration;

pub fn process_image(
    request: RemoteVisionProcessImageRequest,
) -> HandlerResult<RemoteVisionProcessImageResult> {
    if request.endpoint.trim().is_empty() {
        return Err(HandlerError::new(
            "REMOTE_VISION_ENDPOINT_EMPTY",
            "endpoint cannot be empty",
        ));
    }
    if request.model.trim().is_empty() {
        return Err(HandlerError::new(
            "REMOTE_VISION_MODEL_EMPTY",
            "model cannot be empty",
        ));
    }
    if request.image_path.trim().is_empty() {
        return Err(HandlerError::new(
            "REMOTE_VISION_IMAGE_EMPTY",
            "imagePath cannot be empty",
        ));
    }
    if request.instruction.trim().is_empty() {
        return Err(HandlerError::new(
            "REMOTE_VISION_INSTRUCTION_EMPTY",
            "instruction cannot be empty",
        ));
    }
    let temperature = request.temperature.unwrap_or(0.5);
    let max_tokens = request.max_tokens.unwrap_or(1024);
    let content = match request.provider {
        RemoteVisionProvider::OpenAi => OpenAiVisionClient::new(OpenAiConfig {
            endpoint: request.endpoint.clone(),
            api_key: request.api_key.clone(),
            timeout: Duration::from_secs(300),
        })
        .map_err(error)?
        .process_image(
            &request.model,
            &request.image_path,
            &request.instruction,
            temperature,
            max_tokens,
        )
        .map_err(error)?,
        RemoteVisionProvider::Gemini => GeminiVisionClient::new(GeminiConfig {
            endpoint: request.endpoint.clone(),
            api_key: request.api_key.clone(),
            timeout: Duration::from_secs(300),
        })
        .map_err(error)?
        .process_image(
            &request.model,
            &request.image_path,
            &request.instruction,
            temperature,
            max_tokens,
        )
        .map_err(error)?,
    };
    Ok(RemoteVisionProcessImageResult {
        provider: request.provider,
        model: request.model,
        image_path: request.image_path,
        content,
    })
}
fn error(error: impl std::fmt::Display) -> HandlerError {
    HandlerError::new("REMOTE_VISION_ERROR", error.to_string())
}
