use serde::{Deserialize, Serialize};

pub use config_store::Config;

pub const PROTOCOL_VERSION: u32 = 1;

pub mod error_codes {
    pub const BAD_MESSAGE: &str = "BAD_MESSAGE";
    pub const INVALID_PROTOCOL_VERSION: &str = "INVALID_PROTOCOL_VERSION";
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EchoRequest {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AaaRequest {
    pub value: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CheckForUpdateRequest {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFileRequest {
    pub url: String,
    pub destination: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ReadConfigRequest {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteConfigRequest {
    pub config: Box<Config>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathInfoRequest {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanWdModelsRequest {
    pub root: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanEmbeddingModelsRequest {
    pub root: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingExecutionProvider {
    #[default]
    Auto,
    DirectMl,
    Cpu,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProvider {
    #[default]
    Local,
    OpenAi,
    Gemini,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingLoadRequest {
    pub session_id: String,
    pub model_key: String,
    #[serde(default)]
    pub provider: EmbeddingProvider,
    #[serde(default)]
    pub model_path: String,
    #[serde(default)]
    pub tokenizer_path: String,
    pub database_path: String,
    pub namespace: String,
    #[serde(default)]
    pub execution_provider: EmbeddingExecutionProvider,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub remote_model: String,
    #[serde(default)]
    pub remote_dimension: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingImageInput {
    pub id: String,
    pub path: String,
    #[serde(default)]
    pub name: String,
    pub modified_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingIndexBatchRequest {
    pub session_id: String,
    pub images: Vec<EmbeddingImageInput>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingPruneRequest {
    pub session_id: String,
    pub item_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingHealthRequest {
    pub session_id: String,
    pub item_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingStatusRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSearchTextRequest {
    pub session_id: String,
    pub text: String,
    pub top_k: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSearchImageRequest {
    pub session_id: String,
    pub image_path: String,
    #[serde(default)]
    pub exclude_item_id: String,
    pub top_k: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingUnloadRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAiEmbeddingRequest {
    pub endpoint: String,
    #[serde(default)]
    pub api_key: String,
    pub model: String,
    pub input: serde_json::Value,
    pub dimensions: Option<usize>,
    pub timeout_milliseconds: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WdModelKind {
    Wd,
    Cl,
    Camie,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WdExecutionProvider {
    Auto,
    DirectMl,
    Cpu,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WdTagLanguage {
    #[default]
    En,
    Zh,
    Mix,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTaggerLoadRequest {
    pub session_id: String,
    pub model_path: String,
    pub tags_path: String,
    pub model_kind: WdModelKind,
    pub execution_provider: WdExecutionProvider,
    #[serde(default)]
    pub tagset_path: String,
    #[serde(default)]
    pub language: WdTagLanguage,
    #[serde(default)]
    pub splitter: String,
    #[serde(default)]
    pub filter_tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTaggerImage {
    pub id: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTaggerEnqueueRequest {
    pub session_id: String,
    pub image: WdTaggerImage,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTaggerBatchCompleteRequest {
    pub session_id: String,
    pub threshold: f32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTaggerVideoRequest {
    pub session_id: String,
    pub video_path: String,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub frame_count: usize,
    pub batch_size: usize,
    pub threshold: f32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTaggerUnloadRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlamafileLoadRequest {
    pub session_id: String,
    pub llamafile_path: String,
    pub model_path: String,
    pub mmproj_path: String,
    #[serde(default)]
    pub scratch_directory: String,
    #[serde(default)]
    pub port: u16,
    pub context_size: Option<usize>,
    #[serde(default)]
    pub gpu: String,
    pub gpu_layers: Option<i32>,
    pub startup_timeout_milliseconds: Option<u64>,
    pub request_timeout_milliseconds: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlamafileProcessImageRequest {
    pub session_id: String,
    pub image_path: String,
    pub instruction: String,
    #[serde(default)]
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<usize>,
    pub repetition_penalty: Option<f32>,
    #[serde(default)]
    pub stop: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlamafileUnloadRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum Command {
    Echo(EchoRequest),
    Aaa(AaaRequest),
    CheckForUpdate(CheckForUpdateRequest),
    DownloadFile(DownloadFileRequest),
    ReadConfig(ReadConfigRequest),
    WriteConfig(WriteConfigRequest),
    PathInfo(PathInfoRequest),
    ScanWdModels(ScanWdModelsRequest),
    ScanEmbeddingModels(ScanEmbeddingModelsRequest),
    EmbeddingLoad(EmbeddingLoadRequest),
    EmbeddingIndexBatch(EmbeddingIndexBatchRequest),
    EmbeddingPrune(EmbeddingPruneRequest),
    EmbeddingHealth(EmbeddingHealthRequest),
    EmbeddingStatus(EmbeddingStatusRequest),
    EmbeddingSearchText(EmbeddingSearchTextRequest),
    EmbeddingSearchImage(EmbeddingSearchImageRequest),
    EmbeddingUnload(EmbeddingUnloadRequest),
    OpenAiEmbedding(OpenAiEmbeddingRequest),
    WdTaggerLoad(WdTaggerLoadRequest),
    WdTaggerEnqueue(WdTaggerEnqueueRequest),
    WdTaggerBatchComplete(WdTaggerBatchCompleteRequest),
    WdTaggerVideo(WdTaggerVideoRequest),
    WdTaggerUnload(WdTaggerUnloadRequest),
    LlamafileLoad(LlamafileLoadRequest),
    LlamafileProcessImage(LlamafileProcessImageRequest),
    LlamafileUnload(LlamafileUnloadRequest),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub protocol_version: u32,
    pub request_id: String,

    #[serde(flatten)]
    pub command: Command,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EchoResult {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AaaResult {
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckForUpdateResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFileResult {
    pub requested_url: String,
    pub final_url: String,
    pub destination: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub average_bytes_per_second: f64,
    pub elapsed_milliseconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFileProgress {
    pub downloaded_bytes: u64,
    pub remaining_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub bytes_per_second: f64,
    pub percentage: Option<f64>,
    pub elapsed_milliseconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadConfigResult {
    pub config: Box<Config>,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteConfigResult {
    pub config: Box<Config>,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathInfoResult {
    pub path: String,
    pub exists: bool,
    pub is_file: bool,
    pub is_directory: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdModelInfo {
    pub name: String,
    pub model_kind: WdModelKind,
    pub model_path: String,
    pub tags_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanWdModelsResult {
    pub models_directory: String,
    pub models: Vec<WdModelInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingModelInfo {
    pub name: String,
    pub model_key: String,
    pub model_path: String,
    pub tokenizer_path: String,
    pub dimension: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanEmbeddingModelsResult {
    pub models_directory: String,
    pub models: Vec<EmbeddingModelInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingLoadResult {
    pub session_id: String,
    pub model_key: String,
    pub indexed_count: u64,
    pub reused: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingImageFailure {
    pub id: String,
    pub path: String,
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingIndexBatchResult {
    pub session_id: String,
    pub indexed_ids: Vec<String>,
    pub skipped_ids: Vec<String>,
    pub failures: Vec<EmbeddingImageFailure>,
    pub total_indexed: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingPruneResult {
    pub session_id: String,
    pub removed_count: u64,
    pub total_indexed: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingHealthItem {
    pub item_id: String,
    pub source_uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingHealthResult {
    pub session_id: String,
    pub library_count: u64,
    pub indexed_count: u64,
    pub missing_item_ids: Vec<String>,
    pub stale_items: Vec<EmbeddingHealthItem>,
    pub missing_files: Vec<EmbeddingHealthItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingStatusResult {
    pub session_id: String,
    pub model_key: String,
    pub indexed_count: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSearchHit {
    pub item_id: String,
    pub name: String,
    pub source_uri: String,
    pub similarity: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSearchResult {
    pub session_id: String,
    pub hits: Vec<EmbeddingSearchHit>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingUnloadResult {
    pub session_id: String,
    pub removed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAiEmbeddingVector {
    pub index: usize,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAiEmbeddingUsage {
    pub prompt_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAiEmbeddingResult {
    pub model: String,
    pub dimensions: usize,
    pub data: Vec<OpenAiEmbeddingVector>,
    pub usage: OpenAiEmbeddingUsage,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTaggerLoadResult {
    pub session_id: String,
    pub tag_count: usize,
    pub reused: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTagScore {
    pub name: String,
    pub score: f32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdImagePrediction {
    pub id: String,
    pub path: String,
    pub tags: Vec<WdTagScore>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdImageFailure {
    pub id: String,
    pub path: String,
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTaggerEnqueueResult {
    pub session_id: String,
    pub queued: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTaggerBatchCompleteResult {
    pub session_id: String,
    pub predictions: Vec<WdImagePrediction>,
    pub failures: Vec<WdImageFailure>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdVideoFramePrediction {
    pub frame_number: usize,
    pub timestamp_seconds: f64,
    pub tags: Vec<WdTagScore>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTaggerVideoResult {
    pub session_id: String,
    pub video_path: String,
    pub duration_seconds: f64,
    pub frames: Vec<WdVideoFramePrediction>,
    pub tags: Vec<WdTagScore>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTaggerUnloadResult {
    pub session_id: String,
    pub removed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlamafileLoadResult {
    pub session_id: String,
    pub port: u16,
    pub reused: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlamafileProcessImageResult {
    pub session_id: String,
    pub image_path: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlamafileUnloadResult {
    pub session_id: String,
    pub removed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
}

impl ErrorPayload {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum ResultPayload {
    Echo(EchoResult),
    Aaa(AaaResult),
    CheckForUpdate(CheckForUpdateResult),
    DownloadFile(DownloadFileResult),
    ReadConfig(ReadConfigResult),
    WriteConfig(WriteConfigResult),
    PathInfo(PathInfoResult),
    ScanWdModels(ScanWdModelsResult),
    ScanEmbeddingModels(ScanEmbeddingModelsResult),
    EmbeddingLoad(EmbeddingLoadResult),
    EmbeddingIndexBatch(EmbeddingIndexBatchResult),
    EmbeddingPrune(EmbeddingPruneResult),
    EmbeddingHealth(EmbeddingHealthResult),
    EmbeddingStatus(EmbeddingStatusResult),
    EmbeddingSearchText(EmbeddingSearchResult),
    EmbeddingSearchImage(EmbeddingSearchResult),
    EmbeddingUnload(EmbeddingUnloadResult),
    OpenAiEmbedding(OpenAiEmbeddingResult),
    WdTaggerLoad(WdTaggerLoadResult),
    WdTaggerEnqueue(WdTaggerEnqueueResult),
    WdTaggerBatchComplete(WdTaggerBatchCompleteResult),
    WdTaggerVideo(WdTaggerVideoResult),
    WdTaggerUnload(WdTaggerUnloadResult),
    LlamafileLoad(LlamafileLoadResult),
    LlamafileProcessImage(LlamafileProcessImageResult),
    LlamafileUnload(LlamafileUnloadResult),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum ProgressPayload {
    DownloadFile(DownloadFileProgress),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ResponseMessage {
    Result(ResultPayload),
    Error(ErrorPayload),
    Progress(ProgressPayload),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub protocol_version: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,

    #[serde(flatten)]
    pub message: ResponseMessage,
}

impl Response {
    pub fn result(request_id: impl Into<String>, payload: ResultPayload) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: Some(request_id.into()),
            message: ResponseMessage::Result(payload),
        }
    }

    pub fn error(
        request_id: Option<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            message: ResponseMessage::Error(ErrorPayload::new(code, message)),
        }
    }

    pub fn progress(request_id: impl Into<String>, payload: ProgressPayload) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: Some(request_id.into()),
            message: ResponseMessage::Progress(payload),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_echo_request() {
        let json = r#"{
            "protocolVersion": 1,
            "requestId": "r1",
            "type": "echo",
            "payload": {
                "message": "hello"
            }
        }"#;

        let request: Request = serde_json::from_str(json).unwrap();

        assert_eq!(request.protocol_version, 1);
        assert_eq!(request.request_id, "r1");

        match request.command {
            Command::Echo(payload) => {
                assert_eq!(payload.message, "hello");
            }
            _ => panic!("expected echo command"),
        }
    }

    #[test]
    fn parses_aaa_request() {
        let json = r#"{
            "protocolVersion": 1,
            "requestId": "r2",
            "type": "aaa",
            "payload": {
                "value": "aaa"
            }
        }"#;

        let request: Request = serde_json::from_str(json).unwrap();

        match request.command {
            Command::Aaa(payload) => assert_eq!(payload.value, "aaa"),
            _ => panic!("expected aaa command"),
        }
    }

    #[test]
    fn parses_check_for_update_request() {
        let json = r#"{
            "protocolVersion": 1,
            "requestId": "update-check",
            "type": "check_for_update",
            "payload": {}
        }"#;

        let request: Request = serde_json::from_str(json).unwrap();

        assert!(matches!(request.command, Command::CheckForUpdate(_)));
    }

    #[test]
    fn parses_wd_tagger_load_request() {
        let json = r#"{
            "protocolVersion": 1,
            "requestId": "wd-load",
            "type": "wd_tagger_load",
            "payload": {
                "sessionId": "main",
                "modelPath": "model.onnx",
                "tagsPath": "selected_tags.csv",
                "modelKind": "wd",
                "executionProvider": "direct_ml"
            }
        }"#;

        let request: Request = serde_json::from_str(json).unwrap();

        match request.command {
            Command::WdTaggerLoad(payload) => {
                assert_eq!(payload.session_id, "main");
                assert_eq!(payload.model_kind, WdModelKind::Wd);
                assert_eq!(payload.execution_provider, WdExecutionProvider::DirectMl);
            }
            _ => panic!("expected wd_tagger_load command"),
        }
    }

    #[test]
    fn parses_minimal_llamafile_load_request() {
        let json = r#"{
            "protocolVersion": 1,
            "requestId": "llamafile-load",
            "type": "llamafile_load",
            "payload": {
                "sessionId": "main",
                "llamafilePath": "llamafile.exe",
                "modelPath": "model.gguf",
                "mmprojPath": "mmproj.gguf"
            }
        }"#;

        let request: Request = serde_json::from_str(json).unwrap();

        match request.command {
            Command::LlamafileLoad(payload) => {
                assert_eq!(payload.session_id, "main");
                assert_eq!(payload.llamafile_path, "llamafile.exe");
                assert_eq!(payload.context_size, None);
                assert_eq!(payload.gpu_layers, None);
            }
            _ => panic!("expected llamafile load command"),
        }
    }

    #[test]
    fn parses_embedding_index_batch_request() {
        let json = r#"{
            "protocolVersion": 1,
            "requestId": "embedding-batch",
            "type": "embedding_index_batch",
            "payload": {
                "sessionId": "main",
                "images": [{
                    "id": "item-1",
                    "path": "image.jpg",
                    "name": "Image",
                    "modifiedAt": 123
                }]
            }
        }"#;

        let request: Request = serde_json::from_str(json).unwrap();
        match request.command {
            Command::EmbeddingIndexBatch(payload) => {
                assert_eq!(payload.session_id, "main");
                assert_eq!(payload.images.len(), 1);
                assert_eq!(payload.images[0].modified_at, 123);
            }
            _ => panic!("expected embedding index batch command"),
        }
    }

    #[test]
    fn parses_openai_embedding_request_with_json_input() {
        let json = r#"{
            "protocolVersion": 1,
            "requestId": "remote-embedding",
            "type": "open_ai_embedding",
            "payload": {
                "endpoint": "https://example.com/v1/embeddings",
                "apiKey": "secret",
                "model": "gemini-embedding-2-preview",
                "input": [{
                    "role": "user",
                    "content": [{"type": "input_text", "text": "hello"}]
                }],
                "dimensions": 768
            }
        }"#;

        let request: Request = serde_json::from_str(json).unwrap();
        match request.command {
            Command::OpenAiEmbedding(payload) => {
                assert_eq!(payload.model, "gemini-embedding-2-preview");
                assert_eq!(payload.dimensions, Some(768));
                assert!(payload.input.is_array());
                assert_eq!(payload.timeout_milliseconds, None);
            }
            _ => panic!("expected open_ai_embedding command"),
        }
    }

    #[test]
    fn parses_remote_embedding_session_without_local_paths() {
        let json = r#"{
            "protocolVersion": 1,
            "requestId": "remote-load",
            "type": "embedding_load",
            "payload": {
                "sessionId": "remote",
                "modelKey": "openai:remote-model",
                "provider": "open_ai",
                "databasePath": "semantic-index.sqlite3",
                "namespace": "library",
                "endpoint": "https://example.com/v1/embeddings",
                "apiKey": "secret",
                "remoteModel": "remote-model"
            }
        }"#;

        let request: Request = serde_json::from_str(json).unwrap();
        match request.command {
            Command::EmbeddingLoad(payload) => {
                assert_eq!(payload.provider, EmbeddingProvider::OpenAi);
                assert_eq!(payload.remote_model, "remote-model");
                assert!(payload.model_path.is_empty());
                assert!(payload.tokenizer_path.is_empty());
            }
            _ => panic!("expected embedding_load command"),
        }
    }

    #[test]
    fn parses_gemini_embedding_session_with_dimension() {
        let json = r#"{
            "protocolVersion": 1,
            "requestId": "gemini-load",
            "type": "embedding_load",
            "payload": {
                "sessionId": "gemini",
                "modelKey": "gemini:gemini-embedding-2-preview:1536",
                "provider": "gemini",
                "databasePath": "semantic-index.sqlite3",
                "namespace": "library",
                "endpoint": "https://generativelanguage.googleapis.com",
                "apiKey": "secret",
                "remoteModel": "gemini-embedding-2-preview",
                "remoteDimension": 1536
            }
        }"#;

        let request: Request = serde_json::from_str(json).unwrap();
        match request.command {
            Command::EmbeddingLoad(payload) => {
                assert_eq!(payload.provider, EmbeddingProvider::Gemini);
                assert_eq!(payload.remote_model, "gemini-embedding-2-preview");
                assert_eq!(payload.remote_dimension, 1536);
            }
            _ => panic!("expected Gemini embedding_load command"),
        }
    }

    #[test]
    fn parses_embedding_prune_request() {
        let json = r#"{
            "protocolVersion": 1,
            "requestId": "prune",
            "type": "embedding_prune",
            "payload": {
                "sessionId": "embedding-main",
                "itemIds": ["one", "two"]
            }
        }"#;

        let request: Request = serde_json::from_str(json).unwrap();
        match request.command {
            Command::EmbeddingPrune(payload) => {
                assert_eq!(payload.session_id, "embedding-main");
                assert_eq!(payload.item_ids, ["one", "two"]);
            }
            _ => panic!("expected embedding_prune command"),
        }
    }

    #[test]
    fn parses_embedding_health_request() {
        let json = r#"{
            "protocolVersion": 1,
            "requestId": "health",
            "type": "embedding_health",
            "payload": {
                "sessionId": "embedding-main",
                "itemIds": ["one", "two"]
            }
        }"#;

        let request: Request = serde_json::from_str(json).unwrap();
        match request.command {
            Command::EmbeddingHealth(payload) => {
                assert_eq!(payload.session_id, "embedding-main");
                assert_eq!(payload.item_ids, ["one", "two"]);
            }
            _ => panic!("expected embedding_health command"),
        }
    }
}
