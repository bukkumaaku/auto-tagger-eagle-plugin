use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use clip_embedding::{
    ClipConfig, EMBEDDING_DIMENSION, ExecutionProvider as ClipExecutionProvider, JinaClip,
};
use gemini_embedding::{
    ClientConfig as GeminiClientConfig, GeminiEmbeddingClient, prepare_image_bytes,
};
use openai_embedding::{ClientConfig, EmbeddingRequest, OpenAiEmbeddingClient};
use protocol::{
    EmbeddingExecutionProvider, EmbeddingHealthItem, EmbeddingHealthRequest, EmbeddingHealthResult,
    EmbeddingImageFailure, EmbeddingIndexBatchRequest, EmbeddingIndexBatchResult,
    EmbeddingIndexTagsRequest, EmbeddingIndexTagsResult, EmbeddingLoadRequest, EmbeddingLoadResult,
    EmbeddingProvider, EmbeddingPruneRequest, EmbeddingPruneResult, EmbeddingPruneTagsRequest,
    EmbeddingPruneTagsResult, EmbeddingSearchHit, EmbeddingSearchImageRequest,
    EmbeddingSearchResult, EmbeddingSearchTextRequest, EmbeddingStatusRequest,
    EmbeddingStatusResult, EmbeddingTagFailure, EmbeddingUnloadRequest, EmbeddingUnloadResult,
};
use serde_json::json;
use text_vector_store::{TextDocumentRecord, TextVectorRecord, TextVectorStore};
use vector_store::{Modality, SearchResult, VectorRecord, VectorStore};

use crate::sessions::{SessionError, SessionManager};

use super::{HandlerError, HandlerResult};

const SESSION_PREFIX: &str = "embedding:";
const SOURCE_KEY: &str = "primary";
const TEXT_QUERY_SOURCE_KEY: &str = "search-query";
const MAX_SEARCH_RESULTS: usize = 4_096;
const OPENAI_IMAGE_CONCURRENCY_LIMIT: usize = 16;
const GEMINI_IMAGE_PREPROCESS_CONCURRENCY: usize = 6;
const GEMINI_IMAGE_REQUEST_CONCURRENCY_LIMIT: usize = 50;

#[derive(Clone, Eq, PartialEq)]
struct EmbeddingSessionSettings {
    model_key: String,
    provider: EmbeddingProvider,
    model_path: String,
    tokenizer_path: String,
    database_path: String,
    namespace: String,
    execution_provider: EmbeddingExecutionProvider,
    endpoint: String,
    api_key: String,
    remote_model: String,
    remote_dimension: usize,
}

struct EmbeddingSession {
    settings: EmbeddingSessionSettings,
    engine: EmbeddingEngine,
    store: VectorStore,
    text_store: TextVectorStore,
}

enum EmbeddingEngine {
    Local(Box<JinaClip>),
    OpenAi(RemoteEmbedding),
    Gemini(GeminiEmbedding),
}

struct RemoteEmbedding {
    client: OpenAiEmbeddingClient,
    model: String,
    dimension: usize,
}

struct GeminiEmbedding {
    client: GeminiEmbeddingClient,
    model: String,
    dimension: usize,
}

pub fn load(
    request: EmbeddingLoadRequest,
    sessions: &SessionManager,
) -> HandlerResult<EmbeddingLoadResult> {
    validate_session_id(&request.session_id)?;
    if request.namespace.trim().is_empty() {
        return Err(HandlerError::new(
            "EMBEDDING_NAMESPACE_EMPTY",
            "namespace cannot be empty",
        ));
    }
    let settings = EmbeddingSessionSettings {
        model_key: request.model_key.trim().to_string(),
        provider: request.provider,
        model_path: request.model_path,
        tokenizer_path: request.tokenizer_path,
        database_path: request.database_path,
        namespace: request.namespace,
        execution_provider: request.execution_provider,
        endpoint: request.endpoint.trim().to_string(),
        api_key: request.api_key,
        remote_model: request.remote_model.trim().to_string(),
        remote_dimension: request.remote_dimension,
    };
    if settings.model_key.is_empty() {
        return Err(HandlerError::new(
            "EMBEDDING_MODEL_KEY_EMPTY",
            "modelKey cannot be empty",
        ));
    }

    let key = session_key(&request.session_id);
    if let Some(handle) = sessions
        .get::<EmbeddingSession>(&key)
        .map_err(session_error)?
    {
        let session = handle.lock().map_err(session_error)?;
        if session.settings != settings {
            return Err(HandlerError::new(
                "EMBEDDING_SESSION_CONFLICT",
                format!(
                    "session `{}` is loaded with different settings; unload it first",
                    request.session_id
                ),
            ));
        }
        return Ok(EmbeddingLoadResult {
            session_id: request.session_id,
            model_key: session.settings.model_key.clone(),
            indexed_count: indexed_count(&session)?,
            tag_indexed_count: tag_indexed_count(&session)?,
            tag_link_count: tag_link_count(&session)?,
            reused: true,
        });
    }

    let handle = sessions
        .get_or_try_init(key, || create_session(settings.clone()))
        .map_err(|error| HandlerError::new("EMBEDDING_LOAD_FAILED", error.to_string()))?;
    let session = handle.lock().map_err(session_error)?;
    Ok(EmbeddingLoadResult {
        session_id: request.session_id,
        model_key: session.settings.model_key.clone(),
        indexed_count: indexed_count(&session)?,
        tag_indexed_count: tag_indexed_count(&session)?,
        tag_link_count: tag_link_count(&session)?,
        reused: false,
    })
}

pub fn index_batch(
    request: EmbeddingIndexBatchRequest,
    sessions: &SessionManager,
) -> HandlerResult<EmbeddingIndexBatchResult> {
    validate_session_id(&request.session_id)?;
    let handle = embedding_session(sessions, &request.session_id)?;
    let mut session = handle.lock().map_err(session_error)?;
    let namespace = session.settings.namespace.clone();
    let mut skipped_ids = Vec::new();
    let mut failures = Vec::new();
    let mut pending = Vec::new();

    for image in request.images {
        if image.id.trim().is_empty() || image.path.trim().is_empty() {
            failures.push(EmbeddingImageFailure {
                id: image.id,
                path: image.path,
                error: "image id and path cannot be empty".to_string(),
            });
            continue;
        }
        let stored_at = session
            .store
            .item_updated_at(&namespace, &image.id, Modality::Image, SOURCE_KEY)
            .map_err(store_error)?;
        if stored_at.is_some_and(|stored_at| stored_at >= image.modified_at) {
            skipped_ids.push(image.id);
        } else {
            pending.push(image);
        }
    }

    let mut embedded = Vec::new();
    if !pending.is_empty() {
        if session.engine.is_remote() {
            let paths = pending
                .iter()
                .map(|image| PathBuf::from(&image.path))
                .collect::<Vec<_>>();
            for (image, result) in pending
                .into_iter()
                .zip(session.engine.embed_remote_images(&paths))
            {
                match result {
                    Ok(embedding) => embedded.push((image, embedding)),
                    Err(error) => failures.push(EmbeddingImageFailure {
                        id: image.id,
                        path: image.path,
                        error,
                    }),
                }
            }
        } else {
            let paths = pending
                .iter()
                .map(|image| PathBuf::from(&image.path))
                .collect::<Vec<_>>();
            match session.engine.embed_images(&paths) {
                Ok(embeddings) if embeddings.len() == pending.len() => {
                    embedded.extend(pending.into_iter().zip(embeddings));
                }
                Ok(embeddings) => {
                    let batch_error = format!(
                        "batch returned {} embeddings for {} images",
                        embeddings.len(),
                        pending.len()
                    );
                    embed_pending_individually(
                        &mut session.engine,
                        pending,
                        &mut embedded,
                        &mut failures,
                        &batch_error,
                    );
                }
                Err(batch_error) => {
                    embed_pending_individually(
                        &mut session.engine,
                        pending,
                        &mut embedded,
                        &mut failures,
                        &batch_error,
                    );
                }
            }
        }
    }

    let indexed_ids = embedded
        .iter()
        .map(|(image, _)| image.id.clone())
        .collect::<Vec<_>>();
    let records = embedded
        .into_iter()
        .map(|(image, embedding)| VectorRecord {
            namespace: namespace.clone(),
            item_id: image.id,
            modality: Modality::Image,
            source_key: SOURCE_KEY.to_string(),
            source_uri: Some(image.path),
            content: (!image.name.is_empty()).then_some(image.name),
            updated_at: image.modified_at,
            embedding,
        })
        .collect::<Vec<_>>();
    session.store.upsert_many(&records).map_err(store_error)?;
    let total_indexed = indexed_count(&session)?;

    Ok(EmbeddingIndexBatchResult {
        session_id: request.session_id,
        indexed_ids,
        skipped_ids,
        failures,
        total_indexed,
    })
}

pub fn index_tags(
    request: EmbeddingIndexTagsRequest,
    sessions: &SessionManager,
) -> HandlerResult<EmbeddingIndexTagsResult> {
    validate_session_id(&request.session_id)?;
    let handle = embedding_session(sessions, &request.session_id)?;
    let mut session = handle.lock().map_err(session_error)?;
    let session_id = request.session_id.clone();
    let namespace = session.settings.namespace.clone();
    let mut links = Vec::with_capacity(request.items.len());
    let mut unique_tags = HashMap::<String, ()>::new();

    for item in request.items {
        if item.item_id.trim().is_empty() {
            continue;
        }
        let tags = item
            .tags
            .into_iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .collect::<HashSet<_>>();
        let mut tags = tags.into_iter().collect::<Vec<_>>();
        tags.sort();
        for tag in &tags {
            unique_tags.insert(tag.clone(), ());
        }
        links.push((item.item_id, tags));
    }

    let mut documents = Vec::with_capacity(unique_tags.len());
    for tag in unique_tags.keys() {
        let document = TextDocumentRecord {
            namespace: namespace.clone(),
            kind: "tag".to_string(),
            document_id: tag.clone(),
            content: tag.clone(),
            updated_at: 0,
        };
        session
            .text_store
            .upsert_document(&document)
            .map_err(store_error)?;
        if session
            .text_store
            .needs_embedding(&namespace, "tag", tag, tag)
            .map_err(store_error)?
        {
            documents.push(document);
        }
    }

    let mut failures = Vec::new();
    let mut indexed_tags = 0_u64;
    for (document, result) in documents.iter().zip(
        session.engine.embed_texts(
            &documents
                .iter()
                .map(|item| item.content.clone())
                .collect::<Vec<_>>(),
            request.concurrency,
        ),
    ) {
        match result {
            Ok(embedding) => {
                session
                    .text_store
                    .upsert(&TextVectorRecord {
                        document: document.clone(),
                        embedding,
                    })
                    .map_err(store_error)?;
                indexed_tags += 1;
            }
            Err(error) => failures.push(EmbeddingTagFailure {
                tag: document.document_id.clone(),
                error,
            }),
        }
    }

    for (item_id, tags) in links {
        session
            .text_store
            .replace_item_links(&namespace, "tag", &item_id, &tags)
            .map_err(store_error)?;
    }
    let total_tags = tag_indexed_count(&session)?;
    let total_links = tag_link_count(&session)?;
    let skipped_tags = unique_tags.len() as u64 - indexed_tags - failures.len() as u64;

    Ok(EmbeddingIndexTagsResult {
        session_id,
        indexed_tags,
        skipped_tags,
        total_tags,
        total_links,
        failures,
    })
}

pub fn prune_tags(
    request: EmbeddingPruneTagsRequest,
    sessions: &SessionManager,
) -> HandlerResult<EmbeddingPruneTagsResult> {
    validate_session_id(&request.session_id)?;
    let handle = embedding_session(sessions, &request.session_id)?;
    let mut session = handle.lock().map_err(session_error)?;
    let session_id = request.session_id.clone();
    let namespace = session.settings.namespace.clone();
    let removed_tags = session
        .text_store
        .prune_unlinked_documents(&namespace, "tag")
        .map_err(store_error)?;
    Ok(EmbeddingPruneTagsResult {
        session_id,
        removed_tags,
        total_tags: tag_indexed_count(&session)?,
        total_links: tag_link_count(&session)?,
    })
}

fn embed_pending_individually(
    engine: &mut EmbeddingEngine,
    pending: Vec<protocol::EmbeddingImageInput>,
    embedded: &mut Vec<(protocol::EmbeddingImageInput, Vec<f32>)>,
    failures: &mut Vec<EmbeddingImageFailure>,
    batch_error: &str,
) {
    for image in pending {
        match engine.embed_image(&image.path) {
            Ok(embedding) => embedded.push((image, embedding)),
            Err(error) => failures.push(EmbeddingImageFailure {
                id: image.id,
                path: image.path,
                error: format!("{error}; batch error: {batch_error}"),
            }),
        }
    }
}

pub fn prune(
    request: EmbeddingPruneRequest,
    sessions: &SessionManager,
) -> HandlerResult<EmbeddingPruneResult> {
    validate_session_id(&request.session_id)?;
    let keep_ids = request
        .item_ids
        .into_iter()
        .filter(|item_id| !item_id.trim().is_empty())
        .collect::<HashSet<_>>();
    if keep_ids.is_empty() {
        return Err(HandlerError::new(
            "EMBEDDING_PRUNE_EMPTY",
            "itemIds cannot be empty when pruning an embedding index",
        ));
    }
    let handle = embedding_session(sessions, &request.session_id)?;
    let mut session = handle.lock().map_err(session_error)?;
    let namespace = session.settings.namespace.clone();
    let removed_count = session
        .store
        .delete_missing(&namespace, Modality::Image, SOURCE_KEY, &keep_ids)
        .map_err(store_error)?;
    let total_indexed = indexed_count(&session)?;

    Ok(EmbeddingPruneResult {
        session_id: request.session_id,
        removed_count,
        total_indexed,
    })
}

pub fn health(
    request: EmbeddingHealthRequest,
    sessions: &SessionManager,
) -> HandlerResult<EmbeddingHealthResult> {
    validate_session_id(&request.session_id)?;
    let library_ids = request
        .item_ids
        .into_iter()
        .filter(|item_id| !item_id.trim().is_empty())
        .collect::<HashSet<_>>();
    let handle = embedding_session(sessions, &request.session_id)?;
    let session = handle.lock().map_err(session_error)?;
    let stored = session
        .store
        .list_items(&session.settings.namespace, Modality::Image, SOURCE_KEY)
        .map_err(store_error)?;
    let indexed_ids = stored
        .iter()
        .map(|item| item.item_id.clone())
        .collect::<HashSet<_>>();
    let mut missing_item_ids = library_ids
        .iter()
        .filter(|item_id| !indexed_ids.contains(item_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    missing_item_ids.sort();

    let mut stale_items = Vec::new();
    let mut missing_files = Vec::new();
    for item in stored {
        let health_item = EmbeddingHealthItem {
            item_id: item.item_id,
            source_uri: item.source_uri.unwrap_or_default(),
        };
        if !library_ids.contains(&health_item.item_id) {
            stale_items.push(health_item.clone());
        }
        if health_item.source_uri.is_empty()
            || !std::path::Path::new(&health_item.source_uri).is_file()
        {
            missing_files.push(health_item);
        }
    }

    Ok(EmbeddingHealthResult {
        session_id: request.session_id,
        library_count: library_ids.len() as u64,
        indexed_count: indexed_ids.len() as u64,
        missing_item_ids,
        stale_items,
        missing_files,
    })
}

pub fn status(
    request: EmbeddingStatusRequest,
    sessions: &SessionManager,
) -> HandlerResult<EmbeddingStatusResult> {
    validate_session_id(&request.session_id)?;
    let key = session_key(&request.session_id);
    if let Some(handle) = sessions
        .get::<EmbeddingSession>(&key)
        .map_err(session_error)?
    {
        let session = handle.lock().map_err(session_error)?;
        return Ok(EmbeddingStatusResult {
            session_id: request.session_id,
            model_key: session.settings.model_key.clone(),
            indexed_count: indexed_count(&session)?,
            tag_indexed_count: tag_indexed_count(&session)?,
            tag_link_count: tag_link_count(&session)?,
        });
    }

    if request.database_path.trim().is_empty()
        || request.namespace.trim().is_empty()
        || request.model_key.trim().is_empty()
        || request.dimension == 0
    {
        return Err(HandlerError::new(
            "EMBEDDING_SESSION_NOT_FOUND",
            format!("session `{}` is not loaded", request.session_id),
        ));
    }
    let store = VectorStore::open(
        &request.database_path,
        &request.model_key,
        request.dimension,
    )
    .map_err(store_error)?;
    let text_store = TextVectorStore::open(
        &request.database_path,
        &request.model_key,
        request.dimension,
    )
    .map_err(store_error)?;
    Ok(EmbeddingStatusResult {
        session_id: request.session_id,
        model_key: request.model_key,
        indexed_count: store
            .count_modality(&request.namespace, Modality::Image)
            .map_err(store_error)?,
        tag_indexed_count: text_store
            .count_embeddings(&request.namespace, "tag")
            .map_err(store_error)?,
        tag_link_count: text_store
            .count_links(&request.namespace, "tag")
            .map_err(store_error)?,
    })
}

pub fn search_text(
    request: EmbeddingSearchTextRequest,
    sessions: &SessionManager,
) -> HandlerResult<EmbeddingSearchResult> {
    validate_session_id(&request.session_id)?;
    validate_top_k(request.top_k)?;
    let handle = embedding_session(sessions, &request.session_id)?;
    let mut session = handle.lock().map_err(session_error)?;
    let available = indexed_count(&session)? as usize;
    if available == 0 {
        return Ok(EmbeddingSearchResult {
            session_id: request.session_id,
            hits: Vec::new(),
        });
    }
    let text = request.text.trim();
    let namespace = session.settings.namespace.clone();
    let query = if let Some(stored) = session
        .store
        .get_embedding(&namespace, text, Modality::Text, TEXT_QUERY_SOURCE_KEY)
        .map_err(store_error)?
    {
        stored.embedding
    } else {
        let embedding = session
            .engine
            .embed_text(text)
            .map_err(|error| HandlerError::new("EMBEDDING_FAILED", error))?;
        session
            .store
            .upsert(&VectorRecord {
                namespace: namespace.clone(),
                item_id: text.to_string(),
                modality: Modality::Text,
                source_key: TEXT_QUERY_SOURCE_KEY.to_string(),
                source_uri: None,
                content: Some(text.to_string()),
                updated_at: 0,
                embedding: embedding.clone(),
            })
            .map_err(store_error)?;
        embedding
    };
    let fetch_count = request.top_k.min(available);
    let results = session
        .store
        .search(&namespace, Some(Modality::Image), &query, fetch_count)
        .map_err(store_error)?;
    let hits = search_hits(results, None, fetch_count);
    let hits = if request.include_tags {
        merge_tag_hits(&session.text_store, &namespace, &query, hits, fetch_count)
            .map_err(store_error)?
    } else {
        hits
    };

    Ok(EmbeddingSearchResult {
        session_id: request.session_id,
        hits,
    })
}

pub fn search_image(
    request: EmbeddingSearchImageRequest,
    sessions: &SessionManager,
) -> HandlerResult<EmbeddingSearchResult> {
    validate_session_id(&request.session_id)?;
    validate_top_k(request.top_k)?;
    let handle = embedding_session(sessions, &request.session_id)?;
    let mut session = handle.lock().map_err(session_error)?;
    let available = indexed_count(&session)? as usize;
    if available == 0 {
        return Ok(EmbeddingSearchResult {
            session_id: request.session_id,
            hits: Vec::new(),
        });
    }
    let namespace = session.settings.namespace.clone();
    let cached = if request.exclude_item_id.is_empty() {
        None
    } else {
        session
            .store
            .get_embedding(
                &namespace,
                &request.exclude_item_id,
                Modality::Image,
                SOURCE_KEY,
            )
            .map_err(store_error)?
    };
    let query = if let Some(stored) = cached.filter(|stored| {
        stored.source_uri.as_deref() == Some(request.image_path.as_str())
            && stored.updated_at >= request.image_modified_at
    }) {
        stored.embedding
    } else {
        let embedding = session
            .engine
            .embed_image(&request.image_path)
            .map_err(|error| HandlerError::new("EMBEDDING_FAILED", error))?;
        if !request.exclude_item_id.is_empty() {
            session
                .store
                .upsert(&VectorRecord {
                    namespace: namespace.clone(),
                    item_id: request.exclude_item_id.clone(),
                    modality: Modality::Image,
                    source_key: SOURCE_KEY.to_string(),
                    source_uri: Some(request.image_path.clone()),
                    content: None,
                    updated_at: request.image_modified_at,
                    embedding: embedding.clone(),
                })
                .map_err(store_error)?;
        }
        embedding
    };
    let result_count = request.top_k.min(available);
    let fetch_count = image_search_fetch_count(result_count, available);
    let results = session
        .store
        .search(&namespace, Some(Modality::Image), &query, fetch_count)
        .map_err(store_error)?;

    Ok(EmbeddingSearchResult {
        session_id: request.session_id,
        hits: search_hits(
            results,
            (!request.exclude_item_id.is_empty()).then_some(request.exclude_item_id.as_str()),
            result_count,
        ),
    })
}

pub fn unload(
    request: EmbeddingUnloadRequest,
    sessions: &SessionManager,
) -> HandlerResult<EmbeddingUnloadResult> {
    validate_session_id(&request.session_id)?;
    let removed = sessions
        .remove(&session_key(&request.session_id))
        .map_err(session_error)?;
    Ok(EmbeddingUnloadResult {
        session_id: request.session_id,
        removed,
    })
}

fn create_session(settings: EmbeddingSessionSettings) -> Result<EmbeddingSession, String> {
    let engine = match settings.provider {
        EmbeddingProvider::Local => {
            let mut config = ClipConfig::new(&settings.model_path, &settings.tokenizer_path);
            config.execution_provider = match settings.execution_provider {
                EmbeddingExecutionProvider::Auto => ClipExecutionProvider::Auto,
                EmbeddingExecutionProvider::DirectMl => ClipExecutionProvider::DirectMl,
                EmbeddingExecutionProvider::Cpu => ClipExecutionProvider::Cpu,
            };
            EmbeddingEngine::Local(Box::new(
                JinaClip::load(config).map_err(|error| error.to_string())?,
            ))
        }
        EmbeddingProvider::OpenAi => EmbeddingEngine::OpenAi(RemoteEmbedding::load(&settings)?),
        EmbeddingProvider::Gemini => EmbeddingEngine::Gemini(GeminiEmbedding::load(&settings)?),
    };
    let dimension = engine.dimension();
    let store = VectorStore::open(&settings.database_path, &settings.model_key, dimension)
        .map_err(|error| error.to_string())?;
    let text_store = TextVectorStore::open(&settings.database_path, &settings.model_key, dimension)
        .map_err(|error| error.to_string())?;
    Ok(EmbeddingSession {
        settings,
        engine,
        store,
        text_store,
    })
}

impl EmbeddingEngine {
    fn dimension(&self) -> usize {
        match self {
            Self::Local(_) => EMBEDDING_DIMENSION,
            Self::OpenAi(remote) => remote.dimension,
            Self::Gemini(remote) => remote.dimension,
        }
    }

    fn is_remote(&self) -> bool {
        matches!(self, Self::OpenAi(_) | Self::Gemini(_))
    }

    fn embed_text(&mut self, text: &str) -> Result<Vec<f32>, String> {
        match self {
            Self::Local(clip) => clip.embed_text(text).map_err(|error| error.to_string()),
            Self::OpenAi(remote) => remote.embed_text(text),
            Self::Gemini(remote) => remote.embed_text(text),
        }
    }

    fn embed_texts(
        &mut self,
        texts: &[String],
        requested_concurrency: usize,
    ) -> Vec<Result<Vec<f32>, String>> {
        let requested_concurrency = requested_concurrency.max(1);
        match self {
            Self::Local(clip) => texts
                .iter()
                .map(|text| clip.embed_text(text).map_err(|error| error.to_string()))
                .collect(),
            Self::OpenAi(remote) => parallel_map(
                texts,
                requested_concurrency.min(OPENAI_IMAGE_CONCURRENCY_LIMIT),
                |text| remote.embed_text(text),
            ),
            Self::Gemini(remote) => parallel_map(
                texts,
                requested_concurrency.min(GEMINI_IMAGE_REQUEST_CONCURRENCY_LIMIT),
                |text| remote.embed_text(text),
            ),
        }
    }

    fn embed_image(&mut self, path: &str) -> Result<Vec<f32>, String> {
        match self {
            Self::Local(clip) => clip.embed_image(path).map_err(|error| error.to_string()),
            Self::OpenAi(remote) => remote.embed_image(path),
            Self::Gemini(remote) => remote.embed_image(path),
        }
    }

    fn embed_images(&mut self, paths: &[PathBuf]) -> Result<Vec<Vec<f32>>, String> {
        match self {
            Self::Local(clip) => clip.embed_images(paths).map_err(|error| error.to_string()),
            Self::OpenAi(remote) => paths
                .iter()
                .map(|path| remote.embed_image(&path.to_string_lossy()))
                .collect(),
            Self::Gemini(remote) => paths
                .iter()
                .map(|path| remote.embed_image(&path.to_string_lossy()))
                .collect(),
        }
    }

    fn embed_remote_images(&self, paths: &[PathBuf]) -> Vec<Result<Vec<f32>, String>> {
        match self {
            Self::OpenAi(remote) => parallel_map(paths, OPENAI_IMAGE_CONCURRENCY_LIMIT, |path| {
                remote.embed_image(&path.to_string_lossy())
            }),
            Self::Gemini(remote) => remote.embed_images(paths),
            Self::Local(_) => paths
                .iter()
                .map(|_| Err("local embedding engine cannot use remote image mapping".to_string()))
                .collect(),
        }
    }
}

fn parallel_map<T, R, F>(
    items: &[T],
    max_concurrency: usize,
    operation: F,
) -> Vec<Result<R, String>>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> Result<R, String> + Sync,
{
    if items.is_empty() {
        return Vec::new();
    }
    let worker_count = max_concurrency.max(1).min(items.len());
    let chunk_size = items.len().div_ceil(worker_count);
    std::thread::scope(|scope| {
        let handles = items
            .chunks(chunk_size)
            .map(|chunk| {
                let operation = &operation;
                (
                    chunk.len(),
                    scope.spawn(move || chunk.iter().map(operation).collect::<Vec<_>>()),
                )
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .flat_map(|(chunk_len, handle)| {
                handle.join().unwrap_or_else(|_| {
                    (0..chunk_len)
                        .map(|_| Err("remote embedding worker thread panicked".to_string()))
                        .collect()
                })
            })
            .collect()
    })
}

impl RemoteEmbedding {
    fn load(settings: &EmbeddingSessionSettings) -> Result<Self, String> {
        if settings.endpoint.is_empty() {
            return Err("endpoint cannot be empty for an OpenAI embedding session".to_string());
        }
        if settings.remote_model.is_empty() {
            return Err("remoteModel cannot be empty for an OpenAI embedding session".to_string());
        }
        let client =
            OpenAiEmbeddingClient::new(ClientConfig::new(&settings.endpoint, &settings.api_key))
                .map_err(|error| error.to_string())?;
        let mut remote = Self {
            client,
            model: settings.remote_model.clone(),
            dimension: 0,
        };
        let probe = remote.embed_text("dimension probe")?;
        remote.dimension = probe.len();
        Ok(remote)
    }

    fn embed_text(&self, text: &str) -> Result<Vec<f32>, String> {
        if text.trim().is_empty() {
            return Err("embedding text cannot be empty".to_string());
        }
        self.embed_input(json!(text))
    }

    fn embed_image(&self, path: &str) -> Result<Vec<f32>, String> {
        let bytes =
            fs::read(path).map_err(|error| format!("failed to read image {path}: {error}"))?;
        let mime = image_mime(&bytes).ok_or_else(|| format!("unsupported image format: {path}"))?;
        let data_url = format!("data:{mime};base64,{}", BASE64.encode(bytes));
        self.embed_input(json!([{
            "role": "user",
            "content": [{
                "type": "input_image",
                "image_url": data_url
            }]
        }]))
    }

    fn embed_input(&self, input: serde_json::Value) -> Result<Vec<f32>, String> {
        let response = self
            .client
            .embed(&EmbeddingRequest {
                model: self.model.clone(),
                input,
                dimensions: None,
            })
            .map_err(|error| error.to_string())?;
        if response.data.len() != 1 {
            return Err(format!(
                "embedding API returned {} vectors for one input",
                response.data.len()
            ));
        }
        normalize(response.data.into_iter().next().unwrap().embedding)
    }
}

impl GeminiEmbedding {
    fn load(settings: &EmbeddingSessionSettings) -> Result<Self, String> {
        if settings.endpoint.is_empty() {
            return Err("endpoint cannot be empty for a Gemini embedding session".to_string());
        }
        if settings.remote_model.is_empty() {
            return Err("remoteModel cannot be empty for a Gemini embedding session".to_string());
        }
        if settings.remote_dimension == 0 {
            return Err("remoteDimension must be set for a Gemini embedding session".to_string());
        }
        let client = GeminiEmbeddingClient::new(GeminiClientConfig::new(
            &settings.endpoint,
            &settings.api_key,
        ))
        .map_err(|error| error.to_string())?;
        Ok(Self {
            client,
            model: settings.remote_model.clone(),
            dimension: settings.remote_dimension,
        })
    }

    fn embed_text(&self, text: &str) -> Result<Vec<f32>, String> {
        let embedding = self
            .client
            .embed_text(&self.model, text, self.dimension)
            .map_err(|error| error.to_string())?;
        normalize(embedding)
    }

    fn embed_image(&self, path: &str) -> Result<Vec<f32>, String> {
        let bytes =
            fs::read(path).map_err(|error| format!("failed to read image {path}: {error}"))?;
        let embedding = self
            .client
            .embed_image_bytes(&self.model, &bytes, self.dimension)
            .map_err(|error| error.to_string())?;
        normalize(embedding)
    }

    fn embed_images(&self, paths: &[PathBuf]) -> Vec<Result<Vec<f32>, String>> {
        let prepared = parallel_map(paths, GEMINI_IMAGE_PREPROCESS_CONCURRENCY, |path| {
            let path = path.to_string_lossy();
            let bytes = fs::read(path.as_ref())
                .map_err(|error| format!("failed to read image {path}: {error}"))?;
            prepare_image_bytes(&bytes).map_err(|error| error.to_string())
        });

        let mut results = (0..paths.len()).map(|_| None).collect::<Vec<_>>();
        let mut ready = Vec::new();
        for (index, prepared) in prepared.into_iter().enumerate() {
            match prepared {
                Ok(jpeg) => ready.push((index, jpeg)),
                Err(error) => results[index] = Some(Err(error)),
            }
        }

        let embeddings = parallel_map(
            &ready,
            GEMINI_IMAGE_REQUEST_CONCURRENCY_LIMIT,
            |(_, jpeg)| {
                let embedding = self
                    .client
                    .embed_prepared_image(&self.model, jpeg, self.dimension)
                    .map_err(|error| error.to_string())?;
                normalize(embedding)
            },
        );
        for ((index, _), embedding) in ready.into_iter().zip(embeddings) {
            results[index] = Some(embedding);
        }

        results
            .into_iter()
            .map(|result| result.unwrap_or_else(|| Err("embedding result is missing".to_string())))
            .collect()
    }
}

fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else {
        None
    }
}

fn normalize(mut embedding: Vec<f32>) -> Result<Vec<f32>, String> {
    let norm = embedding
        .iter()
        .map(|value| f64::from(*value).powi(2))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm == 0.0 {
        return Err("embedding has an invalid L2 norm".to_string());
    }
    for value in &mut embedding {
        *value = (f64::from(*value) / norm) as f32;
    }
    Ok(embedding)
}

fn embedding_session(
    sessions: &SessionManager,
    session_id: &str,
) -> HandlerResult<crate::sessions::SessionHandle<EmbeddingSession>> {
    sessions
        .get::<EmbeddingSession>(&session_key(session_id))
        .map_err(session_error)?
        .ok_or_else(|| {
            HandlerError::new(
                "EMBEDDING_SESSION_NOT_FOUND",
                format!("session `{session_id}` is not loaded"),
            )
        })
}

fn indexed_count(session: &EmbeddingSession) -> HandlerResult<u64> {
    session
        .store
        .count_modality(&session.settings.namespace, Modality::Image)
        .map_err(store_error)
}

fn tag_indexed_count(session: &EmbeddingSession) -> HandlerResult<u64> {
    session
        .text_store
        .count_embeddings(&session.settings.namespace, "tag")
        .map_err(store_error)
}

fn tag_link_count(session: &EmbeddingSession) -> HandlerResult<u64> {
    session
        .text_store
        .count_links(&session.settings.namespace, "tag")
        .map_err(store_error)
}

fn merge_tag_hits(
    store: &TextVectorStore,
    namespace: &str,
    query: &[f32],
    mut image_hits: Vec<EmbeddingSearchHit>,
    top_k: usize,
) -> Result<Vec<EmbeddingSearchHit>, text_vector_store::StoreError> {
    if image_hits.is_empty() {
        return Ok(image_hits);
    }
    let tag_count = store.count_embeddings(namespace, "tag")? as usize;
    if tag_count == 0 {
        return Ok(image_hits);
    }
    let tag_results = store.search(namespace, "tag", query, tag_count.min(64))?;
    let candidates = image_hits
        .iter()
        .map(|hit| hit.item_id.as_str())
        .collect::<HashSet<_>>();
    let mut tag_scores = HashMap::<String, f64>::new();
    for (rank, tag) in tag_results.iter().enumerate() {
        let tag_weight = tag.similarity.max(0.0) / (rank as f64 + 1.0);
        if tag_weight <= f64::EPSILON {
            continue;
        }
        for item_id in store.linked_item_ids(tag.document_row_id)? {
            if candidates.contains(item_id.as_str()) {
                *tag_scores.entry(item_id).or_default() += tag_weight;
            }
        }
    }
    let mut ranked_tags = tag_scores.into_iter().collect::<Vec<_>>();
    ranked_tags.sort_by(|left, right| right.1.total_cmp(&left.1));
    let tag_ranks = ranked_tags
        .into_iter()
        .enumerate()
        .map(|(rank, (item_id, _))| (item_id, rank))
        .collect::<HashMap<_, _>>();
    if tag_ranks.is_empty() {
        return Ok(image_hits);
    }
    let mut max_score = 0.0_f64;
    for (rank, hit) in image_hits.iter_mut().enumerate() {
        let image_score = 1.0 / (60.0 + rank as f64);
        let tag_score = tag_ranks
            .get(&hit.item_id)
            .map(|tag_rank| 1.0 / (60.0 + *tag_rank as f64))
            .unwrap_or(0.0);
        let score = image_score * 0.7 + tag_score * 0.3;
        max_score = max_score.max(score);
        hit.similarity = score;
    }
    if max_score > 0.0 {
        for hit in &mut image_hits {
            hit.similarity /= max_score;
        }
    }
    image_hits.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
    image_hits.truncate(top_k);
    Ok(image_hits)
}

fn search_hits(
    results: Vec<SearchResult>,
    excluded_item_id: Option<&str>,
    top_k: usize,
) -> Vec<EmbeddingSearchHit> {
    results
        .into_iter()
        .filter(|result| excluded_item_id != Some(result.item_id.as_str()))
        .take(top_k)
        .map(|result| EmbeddingSearchHit {
            item_id: result.item_id,
            name: result.content.unwrap_or_default(),
            source_uri: result.source_uri.unwrap_or_default(),
            similarity: result.similarity,
        })
        .collect()
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

fn validate_top_k(top_k: usize) -> HandlerResult<()> {
    if (1..=MAX_SEARCH_RESULTS).contains(&top_k) {
        Ok(())
    } else {
        Err(HandlerError::new(
            "INVALID_TOP_K",
            format!("topK must be between 1 and {MAX_SEARCH_RESULTS}"),
        ))
    }
}

fn image_search_fetch_count(result_count: usize, available: usize) -> usize {
    result_count
        .saturating_add(1)
        .min(available)
        .min(MAX_SEARCH_RESULTS)
}

fn session_key(session_id: &str) -> String {
    format!("{SESSION_PREFIX}{session_id}")
}

fn session_error(error: SessionError) -> HandlerError {
    HandlerError::new("SESSION_FAILED", error.to_string())
}

fn store_error(error: impl std::fmt::Display) -> HandlerError {
    HandlerError::new("VECTOR_STORE_FAILED", error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            Arc, Barrier,
            atomic::{AtomicU64, Ordering},
        },
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static NEXT_TEST_FILE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn search_limit_matches_sqlite_vec_knn_maximum() {
        assert!(validate_top_k(MAX_SEARCH_RESULTS).is_ok());
        assert_eq!(
            validate_top_k(MAX_SEARCH_RESULTS + 1).unwrap_err().code,
            "INVALID_TOP_K"
        );
        assert_eq!(validate_top_k(0).unwrap_err().code, "INVALID_TOP_K");
    }

    #[test]
    fn image_search_exclusion_does_not_exceed_sqlite_vec_limit() {
        assert_eq!(
            image_search_fetch_count(MAX_SEARCH_RESULTS, 31_772),
            MAX_SEARCH_RESULTS
        );
        assert_eq!(image_search_fetch_count(100, 31_772), 101);
        assert_eq!(image_search_fetch_count(100, 80), 80);
    }

    #[test]
    fn remote_image_mapping_runs_concurrently_and_preserves_order() {
        let paths = (0..4)
            .map(|index| PathBuf::from(index.to_string()))
            .collect::<Vec<_>>();
        let barrier = Arc::new(Barrier::new(paths.len()));
        let results = parallel_map(&paths, 4, {
            let barrier = Arc::clone(&barrier);
            move |path| {
                barrier.wait();
                let value = path.to_string_lossy().parse::<f32>().unwrap();
                Ok(vec![value])
            }
        });
        let values = results
            .into_iter()
            .map(|result| result.unwrap()[0])
            .collect::<Vec<_>>();
        assert_eq!(values, vec![0.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn rejects_invalid_requests_before_model_loading() {
        let error = load(
            EmbeddingLoadRequest {
                session_id: " ".to_string(),
                model_key: "model".to_string(),
                provider: EmbeddingProvider::Local,
                model_path: "missing.onnx".to_string(),
                tokenizer_path: "missing.json".to_string(),
                database_path: "missing.sqlite3".to_string(),
                namespace: "library".to_string(),
                execution_provider: EmbeddingExecutionProvider::Cpu,
                endpoint: String::new(),
                api_key: String::new(),
                remote_model: String::new(),
                remote_dimension: 0,
            },
            &SessionManager::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, "INVALID_SESSION_ID");

        let error = search_text(
            EmbeddingSearchTextRequest {
                session_id: "missing".to_string(),
                text: "query".to_string(),
                top_k: 0,
                include_tags: false,
            },
            &SessionManager::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, "INVALID_TOP_K");
    }

    #[test]
    fn unload_reports_missing_session() {
        let result = unload(
            EmbeddingUnloadRequest {
                session_id: "missing".to_string(),
            },
            &SessionManager::default(),
        )
        .unwrap();
        assert!(!result.removed);
    }

    #[test]
    fn prune_rejects_an_empty_keep_list_before_accessing_the_session() {
        let error = prune(
            EmbeddingPruneRequest {
                session_id: "missing".to_string(),
                item_ids: vec![" ".to_string()],
            },
            &SessionManager::default(),
        )
        .unwrap_err();

        assert_eq!(error.code, "EMBEDDING_PRUNE_EMPTY");
    }

    #[test]
    fn detects_supported_remote_image_formats() {
        assert_eq!(image_mime(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(image_mime(&[0xff, 0xd8, 0xff, 0x00]), Some("image/jpeg"));
        assert_eq!(image_mime(b"RIFF0000WEBPrest"), Some("image/webp"));
        assert_eq!(image_mime(b"BMrest"), Some("image/bmp"));
        assert_eq!(image_mime(b"GIF89a"), None);
    }

    #[test]
    fn normalizes_remote_embeddings() {
        let embedding = normalize(vec![3.0, 4.0]).unwrap();
        assert!((embedding[0] - 0.6).abs() < 1e-6);
        assert!((embedding[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn remote_session_indexes_and_searches_an_image() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_http_request(&mut stream);
                let embedding = if request.contains("dimension probe") {
                    "[1.0,0.0]"
                } else {
                    assert!(request.contains("input_image") || request.contains("query"));
                    "[0.8,0.6]"
                };
                let body = format!(
                    "{{\"model\":\"remote-model\",\"data\":[{{\"index\":0,\"embedding\":{embedding}}}],\"usage\":{{}}}}"
                );
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        let database_path = test_path("remote.sqlite3");
        let image_path = test_path("remote.png");
        fs::write(&image_path, b"\x89PNG\r\n\x1a\nimage").unwrap();
        let sessions = SessionManager::default();

        let loaded = load(
            EmbeddingLoadRequest {
                session_id: "remote".to_string(),
                model_key: "openai:remote-model".to_string(),
                provider: EmbeddingProvider::OpenAi,
                model_path: String::new(),
                tokenizer_path: String::new(),
                database_path: database_path.to_string_lossy().into_owned(),
                namespace: "library".to_string(),
                execution_provider: EmbeddingExecutionProvider::Auto,
                endpoint: format!("http://{address}/v1/embeddings"),
                api_key: "secret".to_string(),
                remote_model: "remote-model".to_string(),
                remote_dimension: 0,
            },
            &sessions,
        )
        .unwrap();
        assert_eq!(loaded.indexed_count, 0);

        let indexed = index_batch(
            EmbeddingIndexBatchRequest {
                session_id: "remote".to_string(),
                images: vec![protocol::EmbeddingImageInput {
                    id: "image-1".to_string(),
                    path: image_path.to_string_lossy().into_owned(),
                    name: "Remote image".to_string(),
                    modified_at: 1,
                }],
            },
            &sessions,
        )
        .unwrap();
        assert_eq!(indexed.indexed_ids, ["image-1"]);
        assert!(indexed.failures.is_empty());

        let results = search_text(
            EmbeddingSearchTextRequest {
                session_id: "remote".to_string(),
                text: "query".to_string(),
                top_k: 1,
                include_tags: false,
            },
            &sessions,
        )
        .unwrap();
        assert_eq!(results.hits.len(), 1);
        assert_eq!(results.hits[0].item_id, "image-1");
        assert!(results.hits[0].similarity > 0.999);

        let cached_text_results = search_text(
            EmbeddingSearchTextRequest {
                session_id: "remote".to_string(),
                text: "query".to_string(),
                top_k: 1,
                include_tags: false,
            },
            &sessions,
        )
        .unwrap();
        assert_eq!(cached_text_results.hits.len(), 1);
        assert_eq!(cached_text_results.hits[0].item_id, results.hits[0].item_id);
        assert!(
            (cached_text_results.hits[0].similarity - results.hits[0].similarity).abs()
                < f64::EPSILON
        );

        let cached_image_results = search_image(
            EmbeddingSearchImageRequest {
                session_id: "remote".to_string(),
                image_path: image_path.to_string_lossy().into_owned(),
                exclude_item_id: "image-1".to_string(),
                image_modified_at: 1,
                top_k: 1,
            },
            &sessions,
        )
        .unwrap();
        assert!(cached_image_results.hits.is_empty());

        unload(
            EmbeddingUnloadRequest {
                session_id: "remote".to_string(),
            },
            &sessions,
        )
        .unwrap();
        server.join().unwrap();
        fs::remove_file(image_path).unwrap();
        fs::remove_file(database_path).unwrap();
    }

    #[test]
    fn hybrid_search_promotes_an_image_linked_to_a_matching_tag() {
        let mut store = TextVectorStore::open_in_memory("test-model", 2).unwrap();
        store
            .upsert(&TextVectorRecord {
                document: TextDocumentRecord {
                    namespace: "library".to_string(),
                    kind: "tag".to_string(),
                    document_id: "beach".to_string(),
                    content: "beach".to_string(),
                    updated_at: 0,
                },
                embedding: vec![1.0, 0.0],
            })
            .unwrap();
        store
            .replace_item_links("library", "tag", "image-with-tag", &["beach".to_string()])
            .unwrap();
        let hits = vec![
            EmbeddingSearchHit {
                item_id: "image-without-tag".to_string(),
                name: String::new(),
                source_uri: String::new(),
                similarity: 0.9,
            },
            EmbeddingSearchHit {
                item_id: "image-with-tag".to_string(),
                name: String::new(),
                source_uri: String::new(),
                similarity: 0.8,
            },
        ];

        let merged = merge_tag_hits(&store, "library", &[1.0, 0.0], hits, 2).unwrap();

        assert_eq!(merged[0].item_id, "image-with-tag");
        assert_eq!(merged.len(), 2);
    }

    fn test_path(suffix: &str) -> PathBuf {
        let unique = NEXT_TEST_FILE.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "eagle-embedding-{}-{nanos}-{unique}-{suffix}",
            std::process::id()
        ))
    }

    fn read_http_request(stream: &mut impl Read) -> String {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4_096];
        loop {
            let length = stream.read(&mut buffer).unwrap();
            if length == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..length]);
            let Some(header_end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") else {
                continue;
            };
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|value| value.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            if bytes.len() >= header_end + 4 + content_length {
                break;
            }
        }
        String::from_utf8(bytes).unwrap()
    }
}
