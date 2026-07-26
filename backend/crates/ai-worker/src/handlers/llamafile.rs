use std::{convert::Infallible, path::Path, time::Duration};

use llamafile::{ImageRequest, LlamafileConfig, LlamafileError, LlamafileSession};
use protocol::{
    LlamafileLoadRequest, LlamafileLoadResult, LlamafileProcessImageRequest,
    LlamafileProcessImageResult, LlamafileUnloadRequest, LlamafileUnloadResult,
};

use super::{HandlerError, HandlerResult};
use crate::sessions::{SessionError, SessionManager};

const DEFAULT_CONTEXT_SIZE: usize = 8192;
const DEFAULT_GPU_LAYERS: i32 = 9999;
const DEFAULT_STARTUP_TIMEOUT_MS: u64 = 90_000;
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_MAX_TOKENS: usize = 1024;
const DEFAULT_TEMPERATURE: f32 = 0.5;
const DEFAULT_REPETITION_PENALTY: f32 = 1.15;

pub fn load(
    request: LlamafileLoadRequest,
    sessions: &SessionManager,
) -> HandlerResult<LlamafileLoadResult> {
    validate_session_id(&request.session_id)?;
    validate_required_file(
        &request.llamafile_path,
        "LLAMAFILE_NOT_FOUND",
        "llamafile executable",
    )?;
    validate_required_file(&request.model_path, "LLAMAFILE_MODEL_NOT_FOUND", "model")?;
    validate_required_file(&request.mmproj_path, "LLAMAFILE_MMPROJ_NOT_FOUND", "mmproj")?;

    let session_id = request.session_id.clone();
    let key = session_key(&session_id);
    let config = build_config(request);

    if let Some(handle) = sessions
        .get::<LlamafileSession>(&key)
        .map_err(session_error)?
    {
        let session = handle.lock().map_err(session_error)?;
        if !same_session_config(session.config(), &config) {
            return Err(HandlerError::new(
                "LLAMAFILE_SESSION_CONFIG_MISMATCH",
                format!(
                    "llamafile session `{session_id}` is already loaded with different settings"
                ),
            ));
        }
        return Ok(LlamafileLoadResult {
            session_id,
            port: session.port(),
            reused: true,
            active_gpu: session.active_gpu().to_string(),
            fallback_reason: session.fallback_reason().unwrap_or_default().to_string(),
        });
    }

    let initialized = LlamafileSession::start(config).map_err(llamafile_error)?;
    let handle = sessions
        .get_or_try_init(key, || Ok::<_, Infallible>(initialized))
        .map_err(session_error)?;
    let session = handle.lock().map_err(session_error)?;

    Ok(LlamafileLoadResult {
        session_id,
        port: session.port(),
        reused: false,
        active_gpu: session.active_gpu().to_string(),
        fallback_reason: session.fallback_reason().unwrap_or_default().to_string(),
    })
}

pub fn process_image(
    request: LlamafileProcessImageRequest,
    sessions: &SessionManager,
) -> HandlerResult<LlamafileProcessImageResult> {
    validate_session_id(&request.session_id)?;
    let session_id = request.session_id;
    let image_path = request.image_path;
    let handle = sessions
        .get::<LlamafileSession>(&session_key(&session_id))
        .map_err(session_error)?
        .ok_or_else(|| {
            HandlerError::new(
                "LLAMAFILE_SESSION_NOT_FOUND",
                format!("llamafile session `{session_id}` is not loaded"),
            )
        })?;

    let mut image_request = ImageRequest::new(&image_path, request.instruction);
    if !request.model.trim().is_empty() {
        image_request.model = request.model;
    }
    image_request.temperature = request.temperature.unwrap_or(DEFAULT_TEMPERATURE);
    image_request.max_tokens = request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
    image_request.repetition_penalty = request
        .repetition_penalty
        .unwrap_or(DEFAULT_REPETITION_PENALTY);
    if !request.stop.is_empty() {
        image_request.stop = request.stop;
    }

    let response = handle
        .lock()
        .map_err(session_error)?
        .process_image(image_request)
        .map_err(llamafile_error)?;

    Ok(LlamafileProcessImageResult {
        session_id,
        image_path,
        content: response.content,
    })
}

pub fn unload(
    request: LlamafileUnloadRequest,
    sessions: &SessionManager,
) -> HandlerResult<LlamafileUnloadResult> {
    validate_session_id(&request.session_id)?;
    let removed = sessions
        .remove(&session_key(&request.session_id))
        .map_err(session_error)?;

    Ok(LlamafileUnloadResult {
        session_id: request.session_id,
        removed,
    })
}

fn build_config(request: LlamafileLoadRequest) -> LlamafileConfig {
    let mut config = LlamafileConfig::new(
        request.llamafile_path,
        request.model_path,
        request.mmproj_path,
    );
    config.log_path = (!request.log_path.trim().is_empty()).then(|| request.log_path.into());
    config.scratch_directory =
        (!request.scratch_directory.trim().is_empty()).then(|| request.scratch_directory.into());
    config.port = request.port;
    config.context_size = request.context_size.unwrap_or(DEFAULT_CONTEXT_SIZE);
    config.gpu = (!request.gpu.trim().is_empty()).then_some(request.gpu);
    config.gpu_layers = request.gpu_layers.unwrap_or(DEFAULT_GPU_LAYERS);
    config.allow_gpu_fallback = request.allow_gpu_fallback;
    config.startup_timeout = Duration::from_millis(
        request
            .startup_timeout_milliseconds
            .unwrap_or(DEFAULT_STARTUP_TIMEOUT_MS),
    );
    config.request_timeout = Duration::from_millis(
        request
            .request_timeout_milliseconds
            .unwrap_or(DEFAULT_REQUEST_TIMEOUT_MS),
    );
    config
}

fn same_session_config(existing: &LlamafileConfig, requested: &LlamafileConfig) -> bool {
    let mut requested = requested.clone();
    requested.log_path.clone_from(&existing.log_path);
    existing == &requested
}

fn session_key(session_id: &str) -> String {
    format!("llamafile:{session_id}")
}

fn validate_session_id(session_id: &str) -> HandlerResult<()> {
    if session_id.trim().is_empty() {
        Err(HandlerError::new(
            "LLAMAFILE_SESSION_ID_EMPTY",
            "sessionId cannot be empty",
        ))
    } else {
        Ok(())
    }
}

fn validate_required_file(path: &str, code: &str, description: &str) -> HandlerResult<()> {
    if !path.trim().is_empty() && Path::new(path).is_file() {
        Ok(())
    } else {
        Err(HandlerError::new(
            code,
            format!("{description} was not found at {path}"),
        ))
    }
}

fn session_error(error: SessionError) -> HandlerError {
    HandlerError::new("LLAMAFILE_SESSION_ERROR", error.to_string())
}

fn llamafile_error(error: LlamafileError) -> HandlerError {
    let code = match error {
        LlamafileError::ExecutableNotFound(_) => "LLAMAFILE_NOT_FOUND",
        LlamafileError::ModelNotFound(_) => "LLAMAFILE_MODEL_NOT_FOUND",
        LlamafileError::MmprojNotFound(_) => "LLAMAFILE_MMPROJ_NOT_FOUND",
        LlamafileError::ImageNotFound(_) => "LLAMAFILE_IMAGE_NOT_FOUND",
        LlamafileError::UnsupportedImage(_) => "LLAMAFILE_IMAGE_UNSUPPORTED",
        LlamafileError::EmptyPrompt => "LLAMAFILE_INSTRUCTION_EMPTY",
        LlamafileError::PortInUse(_) => "LLAMAFILE_PORT_IN_USE",
        LlamafileError::GpuBackendUnavailable { .. } => "LLAMAFILE_GPU_UNAVAILABLE",
        LlamafileError::ExitedEarly(_) => "LLAMAFILE_EXITED_EARLY",
        LlamafileError::StartupTimeout(_) => "LLAMAFILE_STARTUP_TIMEOUT",
        _ => "LLAMAFILE_ERROR",
    };
    HandlerError::new(code, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_config_ignores_log_path_changes() {
        let mut existing = LlamafileConfig::new("llamafile", "model", "mmproj");
        existing.log_path = Some("logs/2026-07-26.log".into());
        let mut requested = existing.clone();
        requested.log_path = Some("logs/2026-07-27.log".into());

        assert!(same_session_config(&existing, &requested));
        requested.model_path = "other-model".into();
        assert!(!same_session_config(&existing, &requested));
    }

    #[test]
    fn rejects_missing_llamafile_before_creating_session() {
        let sessions = SessionManager::default();
        let error = load(
            LlamafileLoadRequest {
                session_id: "test".to_string(),
                llamafile_path: "missing.exe".to_string(),
                model_path: "missing.gguf".to_string(),
                mmproj_path: "missing-mmproj.gguf".to_string(),
                log_path: String::new(),
                scratch_directory: String::new(),
                port: 0,
                context_size: None,
                gpu: String::new(),
                gpu_layers: None,
                allow_gpu_fallback: true,
                startup_timeout_milliseconds: None,
                request_timeout_milliseconds: None,
            },
            &sessions,
        )
        .unwrap_err();

        assert_eq!(error.code, "LLAMAFILE_NOT_FOUND");
        assert!(sessions.is_empty().unwrap());
    }

    #[test]
    fn process_requires_loaded_session() {
        let error = process_image(
            LlamafileProcessImageRequest {
                session_id: "missing".to_string(),
                image_path: "image.png".to_string(),
                instruction: "describe".to_string(),
                model: String::new(),
                temperature: None,
                max_tokens: None,
                repetition_penalty: None,
                stop: Vec::new(),
            },
            &SessionManager::default(),
        )
        .unwrap_err();

        assert_eq!(error.code, "LLAMAFILE_SESSION_NOT_FOUND");
    }
}
