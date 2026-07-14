pub mod aaa;
pub mod check_update;
pub mod config;
pub mod download_file;
pub mod echo;
pub mod embedding;
pub mod llamafile;
pub mod openai_embedding;
pub mod path_info;
pub mod scan_embedding_models;
pub mod scan_wd_models;
pub mod wd_tagger;

use protocol::{Command, ProgressPayload, ResultPayload};
use thiserror::Error;

use crate::sessions::SessionManager;

#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct HandlerError {
    pub code: String,
    pub message: String,
}

impl HandlerError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

pub type HandlerResult<T> = Result<T, HandlerError>;

pub trait EventEmitter {
    fn progress(&mut self, payload: ProgressPayload) -> HandlerResult<()>;
}

pub trait CommandHandler {
    fn handle(
        &mut self,
        command: Command,
        events: &mut dyn EventEmitter,
    ) -> HandlerResult<ResultPayload>;

    fn shutdown(&mut self) -> HandlerResult<()> {
        Ok(())
    }
}

pub struct BuiltinHandlers {
    sessions: SessionManager,
}

impl BuiltinHandlers {
    pub fn new(sessions: SessionManager) -> Self {
        Self { sessions }
    }

    pub fn sessions(&self) -> &SessionManager {
        &self.sessions
    }
}

impl Default for BuiltinHandlers {
    fn default() -> Self {
        Self::new(SessionManager::default())
    }
}

impl CommandHandler for BuiltinHandlers {
    fn handle(
        &mut self,
        command: Command,
        events: &mut dyn EventEmitter,
    ) -> HandlerResult<ResultPayload> {
        match command {
            Command::Aaa(request) => aaa::handle(request).map(ResultPayload::Aaa),
            Command::CheckForUpdate(request) => {
                check_update::handle(request).map(ResultPayload::CheckForUpdate)
            }
            Command::DownloadFile(request) => {
                download_file::handle(request, events).map(ResultPayload::DownloadFile)
            }
            Command::Echo(request) => echo::handle(request).map(ResultPayload::Echo),
            Command::ReadConfig(_) => config::read().map(ResultPayload::ReadConfig),
            Command::WriteConfig(request) => config::write(request).map(ResultPayload::WriteConfig),
            Command::PathInfo(request) => path_info::handle(request).map(ResultPayload::PathInfo),
            Command::ScanWdModels(request) => {
                scan_wd_models::handle(request).map(ResultPayload::ScanWdModels)
            }
            Command::ScanEmbeddingModels(request) => {
                scan_embedding_models::handle(request).map(ResultPayload::ScanEmbeddingModels)
            }
            Command::EmbeddingLoad(request) => {
                embedding::load(request, &self.sessions).map(ResultPayload::EmbeddingLoad)
            }
            Command::EmbeddingIndexBatch(request) => {
                embedding::index_batch(request, &self.sessions)
                    .map(ResultPayload::EmbeddingIndexBatch)
            }
            Command::EmbeddingPrune(request) => {
                embedding::prune(request, &self.sessions).map(ResultPayload::EmbeddingPrune)
            }
            Command::EmbeddingHealth(request) => {
                embedding::health(request, &self.sessions).map(ResultPayload::EmbeddingHealth)
            }
            Command::EmbeddingStatus(request) => {
                embedding::status(request, &self.sessions).map(ResultPayload::EmbeddingStatus)
            }
            Command::EmbeddingSearchText(request) => {
                embedding::search_text(request, &self.sessions)
                    .map(ResultPayload::EmbeddingSearchText)
            }
            Command::EmbeddingSearchImage(request) => {
                embedding::search_image(request, &self.sessions)
                    .map(ResultPayload::EmbeddingSearchImage)
            }
            Command::EmbeddingUnload(request) => {
                embedding::unload(request, &self.sessions).map(ResultPayload::EmbeddingUnload)
            }
            Command::OpenAiEmbedding(request) => {
                openai_embedding::handle(request).map(ResultPayload::OpenAiEmbedding)
            }
            Command::WdTaggerLoad(request) => {
                wd_tagger::load(request, &self.sessions).map(ResultPayload::WdTaggerLoad)
            }
            Command::WdTaggerEnqueue(request) => {
                wd_tagger::enqueue(request, &self.sessions).map(ResultPayload::WdTaggerEnqueue)
            }
            Command::WdTaggerBatchComplete(request) => {
                wd_tagger::batch_complete(request, &self.sessions)
                    .map(ResultPayload::WdTaggerBatchComplete)
            }
            Command::WdTaggerVideo(request) => {
                wd_tagger::tag_video(request, &self.sessions).map(ResultPayload::WdTaggerVideo)
            }
            Command::WdTaggerUnload(request) => {
                wd_tagger::unload(request, &self.sessions).map(ResultPayload::WdTaggerUnload)
            }
            Command::LlamafileLoad(request) => {
                llamafile::load(request, &self.sessions).map(ResultPayload::LlamafileLoad)
            }
            Command::LlamafileProcessImage(request) => {
                llamafile::process_image(request, &self.sessions)
                    .map(ResultPayload::LlamafileProcessImage)
            }
            Command::LlamafileUnload(request) => {
                llamafile::unload(request, &self.sessions).map(ResultPayload::LlamafileUnload)
            }
        }
    }

    fn shutdown(&mut self) -> HandlerResult<()> {
        self.sessions
            .shutdown()
            .map_err(|error| HandlerError::new("SESSION_SHUTDOWN_FAILED", error.to_string()))
    }
}
