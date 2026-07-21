use super::{EventEmitter, HandlerError, HandlerResult};
use gemini_vision::{ClientConfig as GeminiConfig, GeminiVisionClient};
use openai_vision::{ClientConfig as OpenAiConfig, OpenAiVisionClient};
use protocol::{
    ProgressPayload, RemoteVisionBatchItemResult, RemoteVisionProcessBatchRequest,
    RemoteVisionProcessBatchResult, RemoteVisionProcessImageRequest,
    RemoteVisionProcessImageResult, RemoteVisionProvider,
};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

enum RemoteVisionClient {
    OpenAi(OpenAiVisionClient),
    Gemini(GeminiVisionClient),
}

const REMOTE_VISION_CLIENT_CACHE_CAPACITY: usize = 8;

struct CachedRemoteVisionClient {
    provider: u8,
    endpoint: String,
    api_key: String,
    client: Arc<RemoteVisionClient>,
}

static REMOTE_VISION_CLIENTS: OnceLock<Mutex<VecDeque<CachedRemoteVisionClient>>> = OnceLock::new();

impl RemoteVisionClient {
    fn new(provider: RemoteVisionProvider, endpoint: &str, api_key: &str) -> HandlerResult<Self> {
        match provider {
            RemoteVisionProvider::OpenAi => OpenAiVisionClient::new(OpenAiConfig {
                endpoint: endpoint.to_string(),
                api_key: api_key.to_string(),
                timeout: Duration::from_secs(300),
            })
            .map(Self::OpenAi)
            .map_err(error),
            RemoteVisionProvider::Gemini => GeminiVisionClient::new(GeminiConfig {
                endpoint: endpoint.to_string(),
                api_key: api_key.to_string(),
                timeout: Duration::from_secs(300),
            })
            .map(Self::Gemini)
            .map_err(error),
        }
    }

    fn process_image(
        &self,
        model: &str,
        image_path: &str,
        instruction: &str,
        temperature: f32,
        max_tokens: usize,
    ) -> HandlerResult<String> {
        match self {
            Self::OpenAi(client) => client
                .process_image(model, image_path, instruction, temperature, max_tokens)
                .map_err(error),
            Self::Gemini(client) => client
                .process_image(model, image_path, instruction, temperature, max_tokens)
                .map_err(error),
        }
    }
}

fn cached_client(
    provider: RemoteVisionProvider,
    endpoint: &str,
    api_key: &str,
) -> HandlerResult<Arc<RemoteVisionClient>> {
    let provider_key = match provider {
        RemoteVisionProvider::OpenAi => 0,
        RemoteVisionProvider::Gemini => 1,
    };
    let endpoint = endpoint.trim();
    let api_key = api_key.trim();
    let cache = REMOTE_VISION_CLIENTS.get_or_init(|| Mutex::new(VecDeque::new()));
    let mut clients = cache.lock().map_err(|_| {
        HandlerError::new(
            "REMOTE_VISION_CLIENT_CACHE_FAILED",
            "remote vision client cache is unavailable",
        )
    })?;

    if let Some(index) = clients.iter().position(|entry| {
        entry.provider == provider_key && entry.endpoint == endpoint && entry.api_key == api_key
    }) {
        let entry = clients
            .remove(index)
            .expect("cached remote vision client index must exist");
        let client = Arc::clone(&entry.client);
        clients.push_front(entry);
        return Ok(client);
    }

    let client = Arc::new(RemoteVisionClient::new(provider, endpoint, api_key)?);
    clients.push_front(CachedRemoteVisionClient {
        provider: provider_key,
        endpoint: endpoint.to_string(),
        api_key: api_key.to_string(),
        client: Arc::clone(&client),
    });
    clients.truncate(REMOTE_VISION_CLIENT_CACHE_CAPACITY);
    Ok(client)
}

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
    let client = cached_client(request.provider, &request.endpoint, &request.api_key)?;
    let content = client.process_image(
        &request.model,
        &request.image_path,
        &request.instruction,
        temperature,
        max_tokens,
    )?;
    Ok(RemoteVisionProcessImageResult {
        provider: request.provider,
        model: request.model,
        image_path: request.image_path,
        content,
    })
}

pub fn process_batch(
    request: RemoteVisionProcessBatchRequest,
    events: &mut dyn EventEmitter,
) -> HandlerResult<RemoteVisionProcessBatchResult> {
    if request.images.is_empty() {
        return Ok(RemoteVisionProcessBatchResult {
            provider: request.provider,
            model: request.model,
            results: Vec::new(),
        });
    }

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
    if request.instruction.trim().is_empty() {
        return Err(HandlerError::new(
            "REMOTE_VISION_INSTRUCTION_EMPTY",
            "instruction cannot be empty",
        ));
    }

    let temperature = request.temperature.unwrap_or(0.5);
    let max_tokens = request.max_tokens.unwrap_or(1024);
    let client = cached_client(request.provider, &request.endpoint, &request.api_key)?;
    let results = parallel_map_streaming(
        &request.images,
        request.concurrency,
        |image| match client.process_image(
            &request.model,
            &image.image_path,
            &request.instruction,
            temperature,
            max_tokens,
        ) {
            Ok(content) => RemoteVisionBatchItemResult {
                item_id: image.item_id.clone(),
                image_path: image.image_path.clone(),
                content,
                error: String::new(),
            },
            Err(error) => RemoteVisionBatchItemResult {
                item_id: image.item_id.clone(),
                image_path: image.image_path.clone(),
                content: String::new(),
                error: error.message,
            },
        },
        |result| events.progress(ProgressPayload::RemoteVisionBatchItem(result.clone())),
    )?;

    Ok(RemoteVisionProcessBatchResult {
        provider: request.provider,
        model: request.model,
        results,
    })
}

fn parallel_map_streaming<T, R, F, P>(
    items: &[T],
    max_concurrency: usize,
    operation: F,
    mut on_result: P,
) -> HandlerResult<Vec<R>>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
    P: FnMut(&R) -> HandlerResult<()>,
{
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let worker_count = max_concurrency.max(1).min(32).min(items.len());
    thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel();
        let next_index = Arc::new(AtomicUsize::new(0));
        let handles = (0..worker_count)
            .map(|_| {
                let operation = &operation;
                let sender = sender.clone();
                let next_index = Arc::clone(&next_index);
                scope.spawn(move || {
                    loop {
                        let index = next_index.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(index) else {
                            break;
                        };
                        if sender.send(operation(item)).is_err() {
                            break;
                        }
                    }
                })
            })
            .collect::<Vec<_>>();
        drop(sender);
        let mut results = Vec::with_capacity(items.len());
        let mut callback_error = None;
        for result in receiver {
            if callback_error.is_none() {
                if let Err(error) = on_result(&result) {
                    callback_error = Some(error);
                }
            }
            results.push(result);
        }
        for handle in handles {
            handle.join().map_err(|_| {
                HandlerError::new(
                    "REMOTE_VISION_THREAD_FAILED",
                    "remote vision worker panicked",
                )
            })?;
        }
        if let Some(error) = callback_error {
            return Err(error);
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

    use super::{cached_client, parallel_map_streaming};
    use protocol::RemoteVisionProvider;

    #[test]
    fn cached_client_is_reused_across_requests() {
        let first = cached_client(
            RemoteVisionProvider::OpenAi,
            "https://cache-test.invalid/v1/chat/completions",
            "cache-test-key",
        )
        .unwrap();
        let second = cached_client(
            RemoteVisionProvider::OpenAi,
            " https://cache-test.invalid/v1/chat/completions ",
            " cache-test-key ",
        )
        .unwrap();

        assert!(std::sync::Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn parallel_map_streams_results_while_operations_run_concurrently() {
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let items = [1, 2, 3, 4];

        let mut emitted = Vec::new();
        let results = parallel_map_streaming(
            &items,
            4,
            |item| {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(40));
                active.fetch_sub(1, Ordering::SeqCst);
                item * 2
            },
            |result| {
                emitted.push(*result);
                Ok(())
            },
        )
        .unwrap();

        assert!(peak.load(Ordering::SeqCst) > 1);
        assert_eq!(results.len(), 4);
        assert_eq!(emitted.len(), 4);
        let mut sorted_results = results;
        sorted_results.sort_unstable();
        assert_eq!(sorted_results, [2, 4, 6, 8]);
    }
}
