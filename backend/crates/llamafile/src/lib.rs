//! Starts a local llamafile vision server and serializes image requests through a mutable session.

use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom},
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
    pub log_path: Option<PathBuf>,
    pub scratch_directory: Option<PathBuf>,
    pub port: u16,
    pub context_size: usize,
    pub gpu: Option<String>,
    pub gpu_layers: i32,
    pub allow_gpu_fallback: bool,
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
            log_path: None,
            scratch_directory: None,
            port: 0,
            context_size: 8192,
            gpu: None,
            gpu_layers: 9999,
            allow_gpu_fallback: true,
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

    #[error("failed to create llamafile log directory {path}: {source}")]
    CreateLogDirectory {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to open llamafile log file {path}: {source}")]
    OpenLogFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("llamafile exited before becoming ready with status {0}")]
    ExitedEarly(ExitStatus),

    #[error("{backend} backend is unavailable: {detail}. See llamafile log at {log_path}")]
    GpuBackendUnavailable {
        backend: String,
        detail: String,
        log_path: PathBuf,
    },

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
    active_gpu: String,
    fallback_reason: Option<String>,
}

impl LlamafileSession {
    pub fn start(config: LlamafileConfig) -> Result<Self, LlamafileError> {
        validate_file(&config.executable_path, LlamafileError::ExecutableNotFound)?;
        validate_file(&config.model_path, LlamafileError::ModelNotFound)?;
        validate_file(&config.mmproj_path, LlamafileError::MmprojNotFound)?;

        let attempts = gpu_attempts(&config);
        let mut unavailable = Vec::new();
        for (index, gpu) in attempts.iter().enumerate() {
            let mut attempt = config.clone();
            attempt.gpu = Some(gpu.clone());
            if gpu == "disabled" {
                attempt.gpu_layers = 0;
            }
            match Self::start_once(attempt) {
                Ok(mut session) => {
                    session.config = config;
                    session.fallback_reason = (!unavailable.is_empty()).then(|| {
                        format!(
                            "{}; fell back to {}",
                            unavailable.join("; "),
                            gpu_display_name(&session.active_gpu)
                        )
                    });
                    return Ok(session);
                }
                Err(LlamafileError::GpuBackendUnavailable {
                    backend,
                    detail,
                    log_path,
                }) if index + 1 < attempts.len() => {
                    unavailable.push(format!(
                        "{} unavailable: {detail}",
                        gpu_display_name(&backend)
                    ));
                    let _ = log_path;
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!("GPU attempt list is never empty")
    }

    fn start_once(config: LlamafileConfig) -> Result<Self, LlamafileError> {
        let log_offset = log_length(config.log_path.as_deref());
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
            active_gpu: String::new(),
            fallback_reason: None,
        };

        if let Err(error) = session.wait_until_ready() {
            session.stop_process();
            let output = read_log_since(session.config.log_path.as_deref(), log_offset);
            if gpu_backend_unavailable(&output) {
                let backend = canonical_gpu(session.config.gpu.as_deref());
                return Err(LlamafileError::GpuBackendUnavailable {
                    detail: gpu_unavailable_detail(&backend, &output),
                    backend,
                    log_path: session.config.log_path.clone().unwrap_or_default(),
                });
            }
            return Err(error);
        }
        let output = read_log_since(session.config.log_path.as_deref(), log_offset);
        session.active_gpu = detect_active_gpu(&output)
            .unwrap_or_else(|| active_gpu_fallback_name(session.config.gpu.as_deref()));

        Ok(session)
    }

    pub fn config(&self) -> &LlamafileConfig {
        &self.config
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn active_gpu(&self) -> &str {
        &self.active_gpu
    }

    pub fn fallback_reason(&self) -> Option<&str> {
        self.fallback_reason.as_deref()
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
    let log_file = config.log_path.as_deref().map(open_log_file).transpose()?;
    let stdout = log_file
        .as_ref()
        .map(|file| file.try_clone())
        .transpose()
        .map_err(|source| LlamafileError::OpenLogFile {
            path: config.log_path.clone().unwrap_or_default(),
            source,
        })?;

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
        .stdout(stdout.map_or_else(Stdio::null, Stdio::from))
        .stderr(log_file.map_or_else(Stdio::null, Stdio::from))
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

fn canonical_gpu(gpu: Option<&str>) -> String {
    match gpu.unwrap_or("auto").trim().to_ascii_lowercase().as_str() {
        "" | "auto" => "auto",
        "nvidia" | "cuda" | "cublas" => "nvidia",
        "apple" | "metal" => "apple",
        "vulkan" | "vk" => "vulkan",
        "amd" | "rocm" | "rocblas" | "hip" => "amd",
        "cpu" | "disable" | "disabled" => "disabled",
        other => other,
    }
    .to_string()
}

fn gpu_attempts(config: &LlamafileConfig) -> Vec<String> {
    let requested = canonical_gpu(config.gpu.as_deref());
    if !config.allow_gpu_fallback || matches!(requested.as_str(), "auto" | "disabled") {
        return vec![requested];
    }
    match requested.as_str() {
        "nvidia" | "apple" | "amd" => {
            vec![requested, "vulkan".to_string(), "disabled".to_string()]
        }
        "vulkan" => vec![requested, "disabled".to_string()],
        _ => vec![requested],
    }
}

fn active_gpu_fallback_name(gpu: Option<&str>) -> String {
    match canonical_gpu(gpu).as_str() {
        "disabled" => "cpu".to_string(),
        other => other.to_string(),
    }
}

fn detect_active_gpu(output: &str) -> Option<String> {
    let output = output.to_ascii_lowercase();
    [
        ("nvidia cuda gpu support successfully loaded", "nvidia"),
        ("apple metal gpu support successfully loaded", "apple"),
        ("amd rocm gpu support successfully loaded", "amd"),
        ("vulkan gpu support successfully loaded", "vulkan"),
    ]
    .into_iter()
    .find_map(|(marker, backend)| output.contains(marker).then(|| backend.to_string()))
    .or_else(|| {
        (output.contains("no usable gpu found")
            || output.contains("gpu-layers option will be ignored"))
        .then(|| "cpu".to_string())
    })
}

fn gpu_backend_unavailable(output: &str) -> bool {
    let output = output.to_ascii_lowercase();
    output.contains("fatal error: support for --gpu")
        && output.contains("was explicitly requested, but it wasn't available")
}

fn gpu_unavailable_detail(backend: &str, output: &str) -> String {
    let output = output.to_ascii_lowercase();
    if output.contains("no pre-built gpu library found") {
        if backend == "nvidia" {
            return "the CUDA runtime library is missing; verify the NVIDIA driver and matching ggml-cuda.dll".to_string();
        }
        return "the matching GPU runtime library could not be loaded".to_string();
    }
    if output.contains("no devices detected") || output.contains("failed to initialize cuda") {
        return "the runtime loaded but no compatible GPU was detected; update the GPU driver and verify that the process can access the device".to_string();
    }
    match backend {
        "apple" => "Metal could not be initialized; update macOS or use Vulkan/CPU fallback",
        "vulkan" => "Vulkan could not be initialized; install a compatible graphics driver or use CPU fallback",
        _ => "llamafile reported that the requested backend is not available",
    }
    .to_string()
}

fn gpu_display_name(gpu: &str) -> &str {
    match gpu {
        "nvidia" => "CUDA",
        "apple" => "Metal",
        "vulkan" => "Vulkan",
        "amd" => "ROCm",
        "disabled" | "cpu" => "CPU",
        "auto" => "automatic GPU selection",
        other => other,
    }
}

fn log_length(path: Option<&Path>) -> u64 {
    path.and_then(|path| fs::metadata(path).ok())
        .map_or(0, |metadata| metadata.len())
}

fn read_log_since(path: Option<&Path>, offset: u64) -> String {
    let Some(path) = path else {
        return String::new();
    };
    let Ok(mut file) = fs::File::open(path) else {
        return String::new();
    };
    let length = file.metadata().map_or(0, |metadata| metadata.len());
    if file.seek(SeekFrom::Start(offset.min(length))).is_err() {
        return String::new();
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn open_log_file(path: &Path) -> Result<fs::File, LlamafileError> {
    if let Some(directory) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(directory).map_err(|source| LlamafileError::CreateLogDirectory {
            path: directory.to_path_buf(),
            source,
        })?;
    }

    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| LlamafileError::OpenLogFile {
            path: path.to_path_buf(),
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
    use std::{io::Write, time::SystemTime};

    use super::*;

    #[test]
    fn builds_gpu_fallback_chains() {
        let mut config = LlamafileConfig::new("llamafile", "model", "mmproj");
        config.gpu = Some("metal".to_string());
        assert_eq!(gpu_attempts(&config), ["apple", "vulkan", "disabled"]);

        config.gpu = Some("nvidia".to_string());
        config.allow_gpu_fallback = false;
        assert_eq!(gpu_attempts(&config), ["nvidia"]);

        config.gpu = Some("cpu".to_string());
        config.allow_gpu_fallback = true;
        assert_eq!(gpu_attempts(&config), ["disabled"]);
    }

    #[test]
    fn recognizes_gpu_backend_logs() {
        assert!(gpu_backend_unavailable(
            "fatal error: support for --gpu nvidia was explicitly requested, but it wasn't available"
        ));
        assert_eq!(
            detect_active_gpu("cuda: NVIDIA CUDA GPU support successfully loaded").as_deref(),
            Some("nvidia")
        );
        assert_eq!(
            detect_active_gpu("metal: Apple Metal GPU support successfully loaded").as_deref(),
            Some("apple")
        );
        assert_eq!(
            detect_active_gpu("warning: no usable GPU found").as_deref(),
            Some("cpu")
        );
    }

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

    #[test]
    fn creates_and_appends_to_log_file() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("llamafile-log-{unique}"));
        let log_path = root.join("logs").join("2026-07-26.log");

        writeln!(open_log_file(&log_path).unwrap(), "first").unwrap();
        writeln!(open_log_file(&log_path).unwrap(), "second").unwrap();

        assert_eq!(fs::read_to_string(&log_path).unwrap(), "first\nsecond\n");
        fs::remove_dir_all(root).unwrap();
    }
}
