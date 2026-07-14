use std::collections::{HashMap, HashSet};

use protocol::{
    WdExecutionProvider, WdImageFailure, WdImagePrediction, WdModelKind, WdTagLanguage, WdTagScore,
    WdTaggerBatchCompleteRequest, WdTaggerBatchCompleteResult, WdTaggerEnqueueRequest,
    WdTaggerEnqueueResult, WdTaggerImage, WdTaggerLoadRequest, WdTaggerLoadResult,
    WdTaggerUnloadRequest, WdTaggerUnloadResult, WdTaggerVideoRequest, WdTaggerVideoResult,
    WdVideoFramePrediction,
};
use video_tagger::TagVideoRequest;
use wd_tagger::{ExecutionProvider, ModelKind, WdTagger};

use crate::sessions::{SessionError, SessionManager};

use super::{HandlerError, HandlerResult};

const SESSION_PREFIX: &str = "wd-tagger:";

struct WdTaggerSession {
    tagger: WdTagger,
    queued_images: Vec<WdTaggerImage>,
    formatter: TagFormatter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TagFormatSettings {
    tagset_path: String,
    language: WdTagLanguage,
    splitter: String,
    filter_tags: HashSet<String>,
}

struct TagFormatter {
    settings: TagFormatSettings,
    translations: HashMap<String, String>,
}

pub fn load(
    request: WdTaggerLoadRequest,
    sessions: &SessionManager,
) -> HandlerResult<WdTaggerLoadResult> {
    validate_session_id(&request.session_id)?;
    let key = session_key(&request.session_id);
    let model_kind = model_kind(request.model_kind);
    let execution_provider = execution_provider(request.execution_provider);
    let format_settings = TagFormatSettings {
        tagset_path: request.tagset_path.clone(),
        language: request.language,
        splitter: request.splitter.clone(),
        filter_tags: request.filter_tags.iter().cloned().collect(),
    };

    if let Some(handle) = sessions
        .get::<WdTaggerSession>(&key)
        .map_err(session_error)?
    {
        let session = handle.lock().map_err(session_error)?;
        if session.tagger.model_path() != request.model_path
            || session.tagger.tags_path() != request.tags_path
            || session.tagger.model_kind() != model_kind
            || session.tagger.execution_provider() != execution_provider
            || session.formatter.settings != format_settings
        {
            return Err(HandlerError::new(
                "WD_TAGGER_SESSION_CONFLICT",
                format!(
                    "session `{}` is already loaded with different model settings; unload it first",
                    request.session_id
                ),
            ));
        }

        return Ok(WdTaggerLoadResult {
            session_id: request.session_id,
            tag_count: session.tagger.tag_count(),
            reused: true,
        });
    }

    let handle = sessions
        .get_or_try_init(key, || {
            let tagger = WdTagger::load(
                request.model_path,
                request.tags_path,
                model_kind,
                execution_provider,
            )
            .map_err(|error| error.to_string())?;
            let formatter =
                TagFormatter::load(format_settings).map_err(|error| error.to_string())?;
            Ok::<_, String>(WdTaggerSession {
                tagger,
                queued_images: Vec::new(),
                formatter,
            })
        })
        .map_err(|error| HandlerError::new("WD_TAGGER_LOAD_FAILED", error.to_string()))?;
    let tag_count = handle.lock().map_err(session_error)?.tagger.tag_count();

    Ok(WdTaggerLoadResult {
        session_id: request.session_id,
        tag_count,
        reused: false,
    })
}

pub fn enqueue(
    request: WdTaggerEnqueueRequest,
    sessions: &SessionManager,
) -> HandlerResult<WdTaggerEnqueueResult> {
    validate_session_id(&request.session_id)?;
    let handle = sessions
        .get::<WdTaggerSession>(&session_key(&request.session_id))
        .map_err(session_error)?
        .ok_or_else(|| {
            HandlerError::new(
                "WD_TAGGER_SESSION_NOT_FOUND",
                format!("session `{}` is not loaded", request.session_id),
            )
        })?;
    let mut session = handle.lock().map_err(session_error)?;
    session.queued_images.push(request.image);

    Ok(WdTaggerEnqueueResult {
        session_id: request.session_id,
        queued: session.queued_images.len(),
    })
}

pub fn batch_complete(
    request: WdTaggerBatchCompleteRequest,
    sessions: &SessionManager,
) -> HandlerResult<WdTaggerBatchCompleteResult> {
    validate_session_id(&request.session_id)?;
    let handle = sessions
        .get::<WdTaggerSession>(&session_key(&request.session_id))
        .map_err(session_error)?
        .ok_or_else(|| {
            HandlerError::new(
                "WD_TAGGER_SESSION_NOT_FOUND",
                format!("session `{}` is not loaded", request.session_id),
            )
        })?;
    let mut session = handle.lock().map_err(session_error)?;

    if session.queued_images.is_empty() {
        return Ok(WdTaggerBatchCompleteResult {
            session_id: request.session_id,
            predictions: Vec::new(),
            failures: Vec::new(),
        });
    }

    let paths = session
        .queued_images
        .iter()
        .map(|image| image.path.clone())
        .collect::<Vec<_>>();
    let batch = session
        .tagger
        .predict_partial(&paths, request.threshold)
        .map_err(|error| HandlerError::new("WD_TAGGER_PREDICT_FAILED", error.to_string()))?;
    let images = std::mem::take(&mut session.queued_images);

    let predictions = images
        .iter()
        .zip(batch.predictions)
        .map(|(image, prediction)| WdImagePrediction {
            id: image.id.clone(),
            path: prediction.path,
            tags: session.formatter.format(prediction.tags),
        })
        .collect();
    let failures = batch
        .failures
        .into_iter()
        .map(|failure| WdImageFailure {
            id: images[failure.index].id.clone(),
            path: failure.path,
            error: failure.error,
        })
        .collect();

    Ok(WdTaggerBatchCompleteResult {
        session_id: request.session_id,
        predictions,
        failures,
    })
}

pub fn tag_video(
    request: WdTaggerVideoRequest,
    sessions: &SessionManager,
) -> HandlerResult<WdTaggerVideoResult> {
    validate_session_id(&request.session_id)?;
    let handle = sessions
        .get::<WdTaggerSession>(&session_key(&request.session_id))
        .map_err(session_error)?
        .ok_or_else(|| {
            HandlerError::new(
                "WD_TAGGER_SESSION_NOT_FOUND",
                format!("session `{}` is not loaded", request.session_id),
            )
        })?;
    let mut session = handle.lock().map_err(session_error)?;
    let prediction = video_tagger::tag_video(
        &mut session.tagger,
        TagVideoRequest {
            video_path: request.video_path,
            ffmpeg_path: request.ffmpeg_path,
            ffprobe_path: request.ffprobe_path,
            frame_count: request.frame_count,
            batch_size: request.batch_size,
            threshold: request.threshold,
        },
    )
    .map_err(|error| HandlerError::new("VIDEO_TAGGER_FAILED", error.to_string()))?;

    Ok(WdTaggerVideoResult {
        session_id: request.session_id,
        video_path: prediction.video_path,
        duration_seconds: prediction.duration_seconds,
        frames: prediction
            .frames
            .into_iter()
            .map(|frame| WdVideoFramePrediction {
                frame_number: frame.frame_number,
                timestamp_seconds: frame.timestamp_seconds,
                tags: session.formatter.format(frame.tags),
            })
            .collect(),
        tags: session.formatter.format(prediction.tags),
    })
}

pub fn unload(
    request: WdTaggerUnloadRequest,
    sessions: &SessionManager,
) -> HandlerResult<WdTaggerUnloadResult> {
    validate_session_id(&request.session_id)?;
    let removed = sessions
        .remove(&session_key(&request.session_id))
        .map_err(session_error)?;

    Ok(WdTaggerUnloadResult {
        session_id: request.session_id,
        removed,
    })
}

fn validate_session_id(session_id: &str) -> HandlerResult<()> {
    if session_id.trim().is_empty() {
        Err(HandlerError::new(
            "INVALID_SESSION_ID",
            "sessionId cannot be empty",
        ))
    } else {
        Ok(())
    }
}

fn session_key(session_id: &str) -> String {
    format!("{SESSION_PREFIX}{session_id}")
}

fn model_kind(value: WdModelKind) -> ModelKind {
    match value {
        WdModelKind::Wd => ModelKind::Wd,
        WdModelKind::Cl => ModelKind::Cl,
        WdModelKind::Camie => ModelKind::Camie,
    }
}

fn execution_provider(value: WdExecutionProvider) -> ExecutionProvider {
    match value {
        WdExecutionProvider::Auto => ExecutionProvider::Auto,
        WdExecutionProvider::DirectMl => ExecutionProvider::DirectMl,
        WdExecutionProvider::Cpu => ExecutionProvider::Cpu,
    }
}

impl TagFormatter {
    fn load(settings: TagFormatSettings) -> HandlerResult<Self> {
        let mut translations = HashMap::new();
        if !settings.tagset_path.is_empty() {
            let mut reader = csv::ReaderBuilder::new()
                .flexible(true)
                .from_path(&settings.tagset_path)
                .map_err(|error| {
                    HandlerError::new(
                        "TAGSET_READ_FAILED",
                        format!("failed to open {}: {error}", settings.tagset_path),
                    )
                })?;
            for record in reader.records() {
                let record = record.map_err(|error| {
                    HandlerError::new(
                        "TAGSET_READ_FAILED",
                        format!("failed to parse {}: {error}", settings.tagset_path),
                    )
                })?;
                if let Some(tag) = record.get(0) {
                    let translation = record.iter().skip(1).collect::<Vec<_>>().join(",");
                    if !translation.is_empty() {
                        translations.insert(tag.to_string(), translation);
                    }
                }
            }
        }
        Ok(Self {
            settings,
            translations,
        })
    }

    fn format(&self, tags: Vec<wd_tagger::TagScore>) -> Vec<WdTagScore> {
        tags.into_iter()
            .filter_map(|tag| {
                let translation = self.translations.get(&tag.name);
                if self.settings.filter_tags.contains(&tag.name)
                    || translation.is_some_and(|value| self.settings.filter_tags.contains(value))
                {
                    return None;
                }
                let name = match (self.settings.language, translation) {
                    (WdTagLanguage::Zh, Some(value)) => value.clone(),
                    (WdTagLanguage::Mix, Some(value)) => {
                        format!("{}{}{}", value, self.settings.splitter, tag.name)
                    }
                    _ => tag.name,
                };
                Some(WdTagScore {
                    name,
                    score: tag.score,
                })
            })
            .collect()
    }
}

fn session_error(error: SessionError) -> HandlerError {
    HandlerError::new("SESSION_FAILED", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_session_id_before_loading_model() {
        let error = load(
            WdTaggerLoadRequest {
                session_id: " ".to_string(),
                model_path: "missing.onnx".to_string(),
                tags_path: "missing.csv".to_string(),
                model_kind: WdModelKind::Wd,
                execution_provider: WdExecutionProvider::Cpu,
                tagset_path: String::new(),
                language: WdTagLanguage::En,
                splitter: String::new(),
                filter_tags: Vec::new(),
            },
            &SessionManager::default(),
        )
        .unwrap_err();

        assert_eq!(error.code, "INVALID_SESSION_ID");
    }

    #[test]
    fn unload_reports_missing_session() {
        let result = unload(
            WdTaggerUnloadRequest {
                session_id: "wd".to_string(),
            },
            &SessionManager::default(),
        )
        .unwrap();

        assert!(!result.removed);
    }

    #[test]
    fn formats_mixed_tags_and_filters_translations() {
        let formatter = TagFormatter {
            settings: TagFormatSettings {
                tagset_path: String::new(),
                language: WdTagLanguage::Mix,
                splitter: "|".to_string(),
                filter_tags: HashSet::from(["城市".to_string()]),
            },
            translations: HashMap::from([
                ("city".to_string(), "城市".to_string()),
                ("building".to_string(), "建筑".to_string()),
            ]),
        };

        let result = formatter.format(vec![
            wd_tagger::TagScore {
                name: "city".to_string(),
                score: 0.9,
            },
            wd_tagger::TagScore {
                name: "building".to_string(),
                score: 0.8,
            },
        ]);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "建筑|building");
    }
}
