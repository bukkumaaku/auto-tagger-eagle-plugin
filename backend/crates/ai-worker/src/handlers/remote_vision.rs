use super::{HandlerError, HandlerResult};
use gemini_vision::{ClientConfig as GeminiConfig, GeminiVisionClient};
use openai_vision::{ClientConfig as OpenAiConfig, OpenAiVisionClient};
use protocol::{
    RemoteVisionBatchItemResult, RemoteVisionProcessBatchRequest, RemoteVisionProcessBatchResult,
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

pub fn process_batch(
    request: RemoteVisionProcessBatchRequest,
) -> HandlerResult<RemoteVisionProcessBatchResult> {
    if request.images.is_empty() {
        return Ok(RemoteVisionProcessBatchResult {
            provider: request.provider,
            model: request.model,
            results: Vec::new(),
        });
    }

    let temperature = request.temperature;
    let max_tokens = request.max_tokens;
    let results = parallel_map(
        &request.images,
        request.concurrency,
        |image| match process_image(RemoteVisionProcessImageRequest {
            provider: request.provider,
            endpoint: request.endpoint.clone(),
            api_key: request.api_key.clone(),
            model: request.model.clone(),
            image_path: image.image_path.clone(),
            instruction: request.instruction.clone(),
            temperature,
            max_tokens,
        }) {
            Ok(result) => RemoteVisionBatchItemResult {
                item_id: image.item_id.clone(),
                image_path: image.image_path.clone(),
                content: result.content,
                error: String::new(),
            },
            Err(error) => RemoteVisionBatchItemResult {
                item_id: image.item_id.clone(),
                image_path: image.image_path.clone(),
                content: String::new(),
                error: error.message,
            },
        },
    )?;

    Ok(RemoteVisionProcessBatchResult {
        provider: request.provider,
        model: request.model,
        results,
    })
}

fn parallel_map<T, R, F>(items: &[T], max_concurrency: usize, operation: F) -> HandlerResult<Vec<R>>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let worker_count = max_concurrency.max(1).min(32).min(items.len());
    let chunk_size = items.len().div_ceil(worker_count);
    std::thread::scope(|scope| {
        let handles = items
            .chunks(chunk_size)
            .map(|chunk| {
                let operation = &operation;
                scope.spawn(move || chunk.iter().map(operation).collect::<Vec<_>>())
            })
            .collect::<Vec<_>>();
        let mut results = Vec::with_capacity(items.len());
        for handle in handles {
            results.extend(handle.join().map_err(|_| {
                HandlerError::new(
                    "REMOTE_VISION_THREAD_FAILED",
                    "remote vision worker panicked",
                )
            })?);
        }
        Ok(results)
    })
}

fn error(error: impl std::fmt::Display) -> HandlerError {
    HandlerError::new("REMOTE_VISION_ERROR", error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        thread,
        time::Duration,
    };

    use super::parallel_map;

    #[test]
    fn parallel_map_runs_operations_concurrently_and_preserves_order() {
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let items = [1, 2, 3, 4];

        let results = parallel_map(&items, 4, |item| {
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(current, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(40));
            active.fetch_sub(1, Ordering::SeqCst);
            item * 2
        })
        .unwrap();

        assert!(peak.load(Ordering::SeqCst) > 1);
        assert_eq!(results, [2, 4, 6, 8]);
    }
}
