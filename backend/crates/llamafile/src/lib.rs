//! Starts a local llamafile vision server and serializes image requests through a mutable session.

use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use image::ImageFormat;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

const HOST: &str = "127.0.0.1";
const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(90);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const HEALTH_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, PartialEq)]
pub struct LlamafileConfig {
    pub executable_path: PathBuf,
    pub model_path: PathBuf,
    pub mmproj_path: PathBuf,
    pub scratch_directory: Option<PathBuf>,
    pub port: u16,
    pub context_size: usize,
    pub gpu: Option<String>,
    pub gpu_layers: i32,
    pub startup_timeout: Duration,
    pub request_timeout: Duration,
}

impl LlamafileConfig {
    pub fn new(
        executable_path: impl Into<PathBuf>,
        model_path: impl Into<PathBuf>,
        mmproj_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            executable_path: executable_path.into(),
            model_path: model_path.into(),
            mmproj_path: mmproj_path.into(),
            scratch_directory: None,
            port: 0,
            context_size: 8192,
            gpu: None,
            gpu_layers: 9999,
            startup_timeout: DEFAULT_STARTUP_TIMEOUT,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageRequest {
    pub image_path: PathBuf,
    pub prompt: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: usize,
    pub repetition_penalty: f32,
    pub stop: Vec<String>,
}

impl ImageRequest {
    pub fn new(image_path: impl Into<PathBuf>, prompt: impl Into<String>) -> Self {
        Self {
            image_path: image_path.into(),
            prompt: prompt.into(),
            model: "local-vision-model".to_string(),
            temperature: 0.5,
            max_tokens: 1024,
            repetition_penalty: 1.15,
            stop: vec![
                "<|im_end|>".to_string(),
                "<|endoftext|>".to_string(),
                "<|im_start|>".to_string(),
                "[DONE]".to_string(),
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageResponse {
    pub content: String,
}

#[derive(Debug, Error)]
pub enum LlamafileError {
    #[error("llamafile executable was not found at {0}")]
    ExecutableNotFound(PathBuf),

    #[error("model file was not found at {0}")]
    ModelNotFound(PathBuf),

    #[error("mmproj file was not found at {0}")]
    MmprojNotFound(PathBuf),

    #[error("image file was not found at {0}")]
    ImageNotFound(PathBuf),

    #[error("unsupported image format at {0}")]
    UnsupportedImage(PathBuf),

    #[error("instruction cannot be empty")]
    EmptyPrompt,

    #[error("failed to create runtime directory {path}: {source}")]
    CreateRuntimeDirectory {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("port {0} is already in use")]
    PortInUse(u16),

    #[error("failed to reserve a local port: {0}")]
    ReservePort(std::io::Error),

    #[error("failed to start llamafile at {path}: {source}")]
    Spawn {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("llamafile exited before becoming ready with status {0}")]
    ExitedEarly(ExitStatus),

    #[error("llamafile did not become ready within {0:?}")]
    StartupTimeout(Duration),

    #[error("failed to read image {path}: {source}")]
    ReadImage {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("llamafile request failed: {0}")]
    Request(reqwest::Error),

    #[error("llamafile returned HTTP {status}: {message}")]
    HttpStatus { status: u16, message: String },

    #[error("llamafile returned an invalid response: {0}")]
    InvalidResponse(String),
}

pub struct LlamafileSession {
    config: LlamafileConfig,
    child: Child,
    client: Client,
    port: u16,
}

impl LlamafileSession {
    pub fn start(config: LlamafileConfig) -> Result<Self, LlamafileError> {
        validate_file(&config.executable_path, LlamafileError::ExecutableNotFound)?;
        validate_file(&config.model_path, LlamafileError::ModelNotFound)?;
        validate_file(&config.mmproj_path, LlamafileError::MmprojNotFound)?;

        let runtime_directory = runtime_directory(&config);
        fs::create_dir_all(&runtime_directory).map_err(|source| {
            LlamafileError::CreateRuntimeDirectory {
                path: runtime_directory.clone(),
                source,
            }
        })?;

        let port = reserve_port(config.port)?;
        let client = Client::builder()
            .timeout(config.request_timeout)
            .build()
            .map_err(LlamafileError::Request)?;
        let child = spawn_server(&config, &runtime_directory, port)?;
        let mut session = Self {
            config,
            child,
            client,
            port,
        };

        if let Err(error) = session.wait_until_ready() {
            session.stop_process();
            return Err(error);
        }

        Ok(session)
    }

    pub fn config(&self) -> &LlamafileConfig {
        &self.config
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn process_image(
        &mut self,
        request: ImageRequest,
    ) -> Result<ImageResponse, LlamafileError> {
        if request.prompt.trim().is_empty() {
            return Err(LlamafileError::EmptyPrompt);
        }
        validate_file(&request.image_path, LlamafileError::ImageNotFound)?;

        if let Some(status) = self.child.try_wait().map_err(|error| {
            LlamafileError::InvalidResponse(format!("failed to inspect llamafile process: {error}"))
        })? {
            return Err(LlamafileError::ExitedEarly(status));
        }

        let image = fs::read(&request.image_path).map_err(|source| LlamafileError::ReadImage {
            path: request.image_path.clone(),
            source,
        })?;
        let mime_type = image_mime_type(&image)
            .ok_or_else(|| LlamafileError::UnsupportedImage(request.image_path.clone()))?;
        let data_url = format!("data:{mime_type};base64,{}", BASE64.encode(image));
        let payload = completion_payload(&request, data_url);

        let response = self
            .client
            .post(self.completions_url())
            .json(&payload)
            .send()
            .map_err(LlamafileError::Request)?;
        let status = response.status();
        if !status.is_success() {
            let message = response
                .text()
                .unwrap_or_else(|error| format!("failed to read error response: {error}"));
            return Err(LlamafileError::HttpStatus {
                status: status.as_u16(),
                message: truncate(&message, 2000),
            });
        }

        let response: CompletionResponse = response
            .json()
            .map_err(|error| LlamafileError::InvalidResponse(error.to_string()))?;
        let content = response
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or_else(|| {
                LlamafileError::InvalidResponse("missing choices[0].message.content".to_string())
            })?;

        Ok(ImageResponse { content })
    }

    fn wait_until_ready(&mut self) -> Result<(), LlamafileError> {
        let started_at = Instant::now();
        let health_client = Client::builder()
            .timeout(Duration::from_secs(1))
            .build()
            .map_err(LlamafileError::Request)?;

        while started_at.elapsed() < self.config.startup_timeout {
            if let Some(status) = self.child.try_wait().map_err(|error| {
                LlamafileError::InvalidResponse(format!(
                    "failed to inspect llamafile process: {error}"
                ))
            })? {
                return Err(LlamafileError::ExitedEarly(status));
            }

            if health_client
                .get(self.health_url())
                .send()
                .is_ok_and(|response| response.status().is_success())
            {
                return Ok(());
            }
            thread::sleep(HEALTH_INTERVAL);
        }

        Err(LlamafileError::StartupTimeout(self.config.startup_timeout))
    }

    fn health_url(&self) -> String {
        format!("http://{HOST}:{}/health", self.port)
    }

    fn completions_url(&self) -> String {
        format!("http://{HOST}:{}/v1/chat/completions", self.port)
    }

    fn shutdown_url(&self) -> String {
        format!("http://{HOST}:{}/shutdown", self.port)
    }

    fn stop_process(&mut self) {
        if self.child.try_wait().ok().flatten().is_some() {
            return;
        }

        let _ = Client::builder()
            .timeout(Duration::from_secs(1))
            .build()
            .and_then(|client| client.post(self.shutdown_url()).send());

        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }

        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn completion_payload(request: &ImageRequest, data_url: String) -> serde_json::Value {
    json!({
        "model": request.model,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": request.prompt },
                { "type": "image_url", "image_url": { "url": data_url } }
            ]
        }],
        "temperature": request.temperature,
        "max_tokens": request.max_tokens,
        "repetition_penalty": request.repetition_penalty,
        "chat_template_kwargs": { "enable_thinking": false },
        "stop": request.stop,
    })
}

impl Drop for LlamafileSession {
    fn drop(&mut self) {
        self.stop_process();
    }
}

#[derive(Debug, Deserialize)]
struct CompletionResponse {
    choices: Vec<CompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct CompletionChoice {
    message: CompletionMessage,
}

#[derive(Debug, Deserialize)]
struct CompletionMessage {
    content: String,
}

fn validate_file(
    path: &Path,
    error: impl FnOnce(PathBuf) -> LlamafileError,
) -> Result<(), LlamafileError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(error(path.to_path_buf()))
    }
}

fn runtime_directory(config: &LlamafileConfig) -> PathBuf {
    config.scratch_directory.clone().unwrap_or_else(|| {
        config
            .executable_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    })
}

fn reserve_port(requested: u16) -> Result<u16, LlamafileError> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), requested);
    let listener = TcpListener::bind(address).map_err(|error| {
        if requested == 0 {
            LlamafileError::ReservePort(error)
        } else {
            LlamafileError::PortInUse(requested)
        }
    })?;
    let port = listener
        .local_addr()
        .map_err(LlamafileError::ReservePort)?
        .port();
    drop(listener);
    Ok(port)
}

fn spawn_server(
    config: &LlamafileConfig,
    runtime_directory: &Path,
    port: u16,
) -> Result<Child, LlamafileError> {
    let mut command = Command::new(&config.executable_path);
    command
        .arg("--server")
        .arg("-m")
        .arg(&config.model_path)
        .arg("--mmproj")
        .arg(&config.mmproj_path)
        .arg("--host")
        .arg(HOST)
        .arg("--port")
        .arg(port.to_string())
        .arg("-c")
        .arg(config.context_size.to_string())
        .arg("-ngl")
        .arg(config.gpu_layers.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(runtime_directory);

    if let Some(gpu) = config.gpu.as_deref().filter(|gpu| !gpu.trim().is_empty()) {
        command.arg("--gpu").arg(gpu);
    }

    apply_runtime_environment(&mut command, runtime_directory);
    command.spawn().map_err(|source| LlamafileError::Spawn {
        path: config.executable_path.clone(),
        source,
    })
}

fn apply_runtime_environment(command: &mut Command, runtime_directory: &Path) {
    for variable in [
        "LLAMAFILE",
        "LLAMAFILE_HOME",
        "LLAMA_CACHE",
        "TMPDIR",
        "TMP",
        "TEMP",
        "HOME",
        "USERPROFILE",
    ] {
        command.env(variable, runtime_directory);
    }

    #[cfg(windows)]
    if let Some(prefix) = runtime_directory.components().next() {
        let drive = prefix.as_os_str().to_string_lossy();
        if drive.ends_with(':') {
            command.env("HOMEDRIVE", drive.as_ref());
        }
    }
}

fn image_mime_type(bytes: &[u8]) -> Option<&'static str> {
    match image::guess_format(bytes).ok()? {
        ImageFormat::Jpeg => Some("image/jpeg"),
        ImageFormat::Png => Some("image/png"),
        ImageFormat::WebP => Some("image/webp"),
        ImageFormat::Bmp => Some("image/bmp"),
        _ => None,
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_llamafile_before_other_paths() {
        let config = LlamafileConfig::new("missing-llamafile", "missing-model", "missing-mmproj");
        let error = match LlamafileSession::start(config) {
            Ok(_) => panic!("missing llamafile unexpectedly started"),
            Err(error) => error,
        };

        assert!(matches!(error, LlamafileError::ExecutableNotFound(_)));
    }

    #[test]
    fn detects_supported_image_mime_types() {
        assert_eq!(image_mime_type(b"\x89PNG\r\n\x1a\n"), Some("image/png"));
        assert_eq!(image_mime_type(b"not an image"), None);
    }

    #[test]
    fn image_request_has_safe_defaults() {
        let request = ImageRequest::new("image.png", "describe it");

        assert_eq!(request.max_tokens, 1024);
        assert_eq!(request.temperature, 0.5);
        assert!(!request.stop.is_empty());
    }

    #[test]
    fn image_request_disables_thinking_mode() {
        let request = ImageRequest::new("image.png", "describe it");
        let payload = completion_payload(&request, "data:image/png;base64,test".to_string());

        assert_eq!(payload["chat_template_kwargs"]["enable_thinking"], false);
    }
}
