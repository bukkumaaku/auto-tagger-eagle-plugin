export const PROTOCOL_VERSION = 1 as const;

export type EchoRequest = {
    message: string;
};

export type CheckForUpdateRequest = Record<string, never>;

export type DownloadFileRequest = {
    url: string;
    destination: string;
};

export type RemoteEmbeddingProfile = { id: string; name: string; provider: "open_ai" | "gemini"; endpoint: string; apiKey: string; model: string; dimension: number; resolvedModelKey?: string };
export type RemoteLlmProfile = { id: string; name: string; provider: "open_ai" | "gemini"; endpoint: string; apiKey: string; model: string };
export type Config = {
    [key: string]: string | number | boolean | string[] | RemoteEmbeddingProfile[] | RemoteLlmProfile[];
    endpoint: string;
    apiKey: string;
    llmProvider: "local" | "open_ai" | "gemini";
    llmEndpoint: string;
    llmApiKey: string;
    llmRemoteModel: string;
    llmRemoteConcurrency: number;
    embeddingProvider: EmbeddingProvider;
    embeddingDimension: number;
    embeddingResolvedModelKey: string;
    embeddingRemoteProfiles: RemoteEmbeddingProfile[];
    embeddingRemoteProfileId: string;
    llmRemoteProfiles: RemoteLlmProfile[];
    llmRemoteProfileId: string;
    modelPath: string;
    threshold: number;
    negativePromptWeight: number;
    steps: number;
    filterTags: string[];
    overwrite: string;
    language: string;
    splitter: string;
    readVideo: string;
    modelLocation: string;
    llmModelPath: string;
    llmRunnerPath: string;
    llmContextSize: number;
    llmNGL: string;
    llmGpu: string;
    llmUseVulkan: boolean;
    llmTemperature: number;
    llmMaxTokens: number;
    llmOverwrite: string;
    llmAnnotationOverwrite: string;
    llmTaggerOrAnnotation: string;
    llmTaggerPrompt: string;
    llmAnnotationPrompt: string;
    embeddingModelId: string;
    embeddingModelName: string;
    embeddingDevice: string;
    embeddingBatchSize: number;
};

export type ReadConfigRequest = Record<string, never>;

export type WriteConfigRequest = {
    config: Config;
};

export type PathInfoRequest = {
    path: string;
};

export type BackupWriteRequest = {
    directory: string;
    filename: string;
    content: string;
};

export type BackupListRequest = {
    directory: string;
};

export type BackupReadRequest = {
    path: string;
};

export type ScanWdModelsRequest = {
    root: string;
};

export type ScanEmbeddingModelsRequest = {
    root: string;
};

export type EmbeddingExecutionProvider = "auto" | "direct_ml" | "cpu";
export type EmbeddingProvider = "local" | "open_ai" | "gemini";

export type EmbeddingLoadRequest = {
    sessionId: string;
    modelKey: string;
    provider: EmbeddingProvider;
    modelPath: string;
    tokenizerPath: string;
    databasePath: string;
    namespace: string;
    executionProvider: EmbeddingExecutionProvider;
    endpoint: string;
    apiKey: string;
    remoteModel: string;
    remoteDimension: number;
    legacyModelKey: string;
};

export type EmbeddingImageInput = {
    id: string;
    path: string;
    name: string;
    annotation?: string;
    modifiedAt: number;
};

export type EmbeddingTagInput = {
    itemId: string;
    tags: string[];
};
export type EmbeddingAnnotationInput = { itemId: string; annotation: string; updatedAt: number };

export type EmbeddingIndexBatchRequest = {
    sessionId: string;
    images: EmbeddingImageInput[];
    force: boolean;
};

export type EmbeddingIndexTagsRequest = {
    sessionId: string;
    items: EmbeddingTagInput[];
    concurrency: number;
    force: boolean;
    forceTagIds: string[];
};
export type EmbeddingIndexAnnotationsRequest = { sessionId: string; items: EmbeddingAnnotationInput[]; concurrency: number; force: boolean };

export type EmbeddingPruneRequest = {
    sessionId: string;
    itemIds: string[];
};

export type EmbeddingPruneTagsRequest = {
    sessionId: string;
    itemIds: string[];
};
export type EmbeddingPruneAnnotationsRequest = { sessionId: string; itemIds: string[] };

export type EmbeddingHealthRequest = {
    sessionId: string;
    itemIds: string[];
    repairLegacyEndpoints?: boolean;
};

export type EmbeddingStatusRequest = {
    sessionId: string;
    databasePath?: string;
    namespace?: string;
    modelKey?: string;
    dimension?: number;
    legacyModelKey?: string;
};
export type EmbeddingMigrateTextRequest = {
    databasePath: string;
    namespace: string;
    modelKey: string;
    dimension?: number;
    legacyModelKey?: string;
};

export type EmbeddingSearchTextRequest = {
    sessionId: string;
    text: string;
    topK: number;
    includeImage?: boolean;
    includeTags?: boolean;
    includeAnnotations?: boolean;
};

export type EmbeddingSearchImageRequest = {
    sessionId: string;
    imagePath: string;
    excludeItemId: string;
    imageModifiedAt: number;
    topK: number;
};

export type EmbeddingUnloadRequest = {
    sessionId: string;
};

export type WdModelKind = "wd" | "cl" | "camie";
export type WdExecutionProvider = "auto" | "direct_ml" | "cpu";
export type WdTagLanguage = "en" | "zh" | "mix";

export type WdTaggerLoadRequest = {
    sessionId: string;
    modelPath: string;
    tagsPath: string;
    modelKind: WdModelKind;
    executionProvider: WdExecutionProvider;
    tagsetPath: string;
    language: WdTagLanguage;
    splitter: string;
    filterTags: string[];
};

export type WdTaggerImage = {
    id: string;
    path: string;
};

export type WdTaggerEnqueueRequest = {
    sessionId: string;
    image: WdTaggerImage;
};

export type WdTaggerBatchCompleteRequest = {
    sessionId: string;
    threshold: number;
};

export type WdTaggerVideoRequest = {
    sessionId: string;
    videoPath: string;
    ffmpegPath: string;
    ffprobePath: string;
    frameCount: number;
    batchSize: number;
    threshold: number;
};

export type WdTaggerUnloadRequest = {
    sessionId: string;
};

export type LlamafileLoadRequest = {
    sessionId: string;
    llamafilePath: string;
    modelPath: string;
    mmprojPath: string;
    scratchDirectory?: string;
    port?: number;
    contextSize?: number;
    gpu?: string;
    gpuLayers?: number;
    startupTimeoutMilliseconds?: number;
    requestTimeoutMilliseconds?: number;
};

export type LlamafileProcessImageRequest = {
    sessionId: string;
    imagePath: string;
    instruction: string;
    model?: string;
    temperature?: number;
    maxTokens?: number;
    repetitionPenalty?: number;
    stop?: string[];
};

export type LlamafileUnloadRequest = {
    sessionId: string;
};
export type VideoExtractFramesRequest = { videoPath: string; ffmpegPath: string; ffprobePath: string };
export type VideoCleanupFramesRequest = { directory: string };
export type RemoteVisionProvider = "open_ai" | "gemini";
export type RemoteVisionProcessImageRequest = { provider: RemoteVisionProvider; endpoint: string; apiKey?: string; model: string; imagePath: string; instruction: string; temperature?: number; maxTokens?: number };
export type RemoteVisionBatchImage = { itemId: string; imagePath: string };
export type RemoteVisionProcessBatchRequest = { provider: RemoteVisionProvider; endpoint: string; apiKey?: string; model: string; images: RemoteVisionBatchImage[]; instruction: string; temperature?: number; maxTokens?: number; concurrency: number };

export type CommandPayloadMap = {
    echo: EchoRequest;
    check_for_update: CheckForUpdateRequest;
    download_file: DownloadFileRequest;
    read_config: ReadConfigRequest;
    write_config: WriteConfigRequest;
    path_info: PathInfoRequest;
    backup_write: BackupWriteRequest;
    backup_list: BackupListRequest;
    backup_read: BackupReadRequest;
    scan_wd_models: ScanWdModelsRequest;
    scan_embedding_models: ScanEmbeddingModelsRequest;
    embedding_load: EmbeddingLoadRequest;
    embedding_index_batch: EmbeddingIndexBatchRequest;
    embedding_index_tags: EmbeddingIndexTagsRequest;
    embedding_index_annotations: EmbeddingIndexAnnotationsRequest;
    embedding_prune: EmbeddingPruneRequest;
    embedding_prune_tags: EmbeddingPruneTagsRequest;
    embedding_prune_annotations: EmbeddingPruneAnnotationsRequest;
    embedding_health: EmbeddingHealthRequest;
    embedding_status: EmbeddingStatusRequest;
    embedding_migrate_text: EmbeddingMigrateTextRequest;
    embedding_search_text: EmbeddingSearchTextRequest;
    embedding_search_image: EmbeddingSearchImageRequest;
    embedding_unload: EmbeddingUnloadRequest;
    wd_tagger_load: WdTaggerLoadRequest;
    wd_tagger_enqueue: WdTaggerEnqueueRequest;
    wd_tagger_batch_complete: WdTaggerBatchCompleteRequest;
    wd_tagger_video: WdTaggerVideoRequest;
    video_extract_frames: VideoExtractFramesRequest;
    video_cleanup_frames: VideoCleanupFramesRequest;
    wd_tagger_unload: WdTaggerUnloadRequest;
    llamafile_load: LlamafileLoadRequest;
    llamafile_process_image: LlamafileProcessImageRequest;
    llamafile_unload: LlamafileUnloadRequest;
    remote_vision_process_image: RemoteVisionProcessImageRequest;
    remote_vision_process_batch: RemoteVisionProcessBatchRequest;
};

export type CommandType = keyof CommandPayloadMap;

export type WorkerRequest<K extends CommandType = CommandType> = {
    [P in K]: {
        protocolVersion: typeof PROTOCOL_VERSION;
        requestId: string;
        type: P;
        payload: CommandPayloadMap[P];
    };
}[K];

export type EchoResult = {
    message: string;
};

export type CheckForUpdateResult = {
    currentVersion: string;
    latestVersion: string;
    updateAvailable: boolean;
    releaseUrl: string;
};

export type DownloadFileResult = {
    requestedUrl: string;
    finalUrl: string;
    destination: string;
    downloadedBytes: number;
    totalBytes: number | null;
    averageBytesPerSecond: number;
    elapsedMilliseconds: number;
};

export type ReadConfigResult = {
    config: Config;
    path: string;
};

export type WriteConfigResult = {
    config: Config;
    path: string;
};

export type PathInfoResult = {
    path: string;
    exists: boolean;
    isFile: boolean;
    isDirectory: boolean;
};

export type WdModelInfo = {
    name: string;
    modelKind: WdModelKind;
    modelPath: string;
    tagsPath: string;
};

export type ScanWdModelsResult = {
    modelsDirectory: string;
    models: WdModelInfo[];
};

export type EmbeddingModelInfo = {
    name: string;
    modelKey: string;
    modelPath: string;
    tokenizerPath: string;
    dimension: number;
};

export type ScanEmbeddingModelsResult = {
    modelsDirectory: string;
    models: EmbeddingModelInfo[];
};

export type EmbeddingLoadResult = {
    sessionId: string;
    modelKey: string;
    dimension: number;
    indexedCount: number;
    tagDocumentCount: number;
    tagIndexedCount: number;
    tagLinkCount: number;
    annotationDocumentCount: number;
    annotationIndexedCount: number;
    reused: boolean;
};

export type BackupWriteResult = { path: string };
export type BackupFileEntry = { name: string; path: string };
export type BackupListResult = { directory: string; entries: BackupFileEntry[] };
export type BackupReadResult = { path: string; content: string };

export type EmbeddingImageFailure = {
    id: string;
    path: string;
    error: string;
};

export type EmbeddingIndexBatchResult = {
    sessionId: string;
    indexedIds: string[];
    skippedIds: string[];
    failures: EmbeddingImageFailure[];
    totalIndexed: number;
};

export type EmbeddingTagFailure = {
    tag: string;
    error: string;
};

export type EmbeddingIndexTagsResult = {
    sessionId: string;
    indexedTags: number;
    skippedTags: number;
    totalTags: number;
    totalLinks: number;
    failures: EmbeddingTagFailure[];
};
export type EmbeddingAnnotationFailure = { itemId: string; error: string };
export type EmbeddingIndexAnnotationsResult = { sessionId: string; indexedAnnotations: number; skippedAnnotations: number; totalAnnotations: number; failures: EmbeddingAnnotationFailure[] };

export type EmbeddingPruneResult = {
    sessionId: string;
    removedCount: number;
    totalIndexed: number;
};

export type EmbeddingPruneTagsResult = {
    sessionId: string;
    removedTags: number;
    totalTags: number;
    totalLinks: number;
};
export type EmbeddingPruneAnnotationsResult = { sessionId: string; removedAnnotations: number; totalAnnotations: number };

export type EmbeddingHealthItem = {
    itemId: string;
    sourceUri: string;
};

export type EmbeddingHealthResult = {
    sessionId: string;
    libraryCount: number;
    indexedCount: number;
    missingItemIds: string[];
    staleItems: EmbeddingHealthItem[];
    missingFiles: EmbeddingHealthItem[];
    removedLegacyModelKeys: string[];
    removedLegacyTableCount: number;
    removedLegacyVectorCount: number;
};

export type EmbeddingStatusResult = {
    sessionId: string;
    modelKey: string;
    indexedCount: number;
    tagDocumentCount: number;
    tagIndexedCount: number;
    tagLinkCount: number;
    annotationDocumentCount: number;
    annotationIndexedCount: number;
    legacyTextModelDetected?: boolean;
    reusableImageCount: number;
    reusableTagCount: number;
    reusableAnnotationCount: number;
};
export type EmbeddingMigrateTextResult = {
    modelKey: string;
    imageIndexedCount: number;
    tagIndexedCount: number;
    annotationIndexedCount: number;
};

export type EmbeddingSearchHit = {
    itemId: string;
    name: string;
    sourceUri: string;
    similarity: number;
};

export type EmbeddingSearchResult = {
    sessionId: string;
    hits: EmbeddingSearchHit[];
};

export type EmbeddingUnloadResult = {
    sessionId: string;
    removed: boolean;
};

export type WdTaggerLoadResult = {
    sessionId: string;
    tagCount: number;
    reused: boolean;
};

export type WdTagScore = {
    name: string;
    score: number;
};

export type WdImagePrediction = {
    id: string;
    path: string;
    tags: WdTagScore[];
};

export type WdImageFailure = {
    id: string;
    path: string;
    error: string;
};

export type WdTaggerEnqueueResult = {
    sessionId: string;
    queued: number;
};

export type WdTaggerBatchCompleteResult = {
    sessionId: string;
    predictions: WdImagePrediction[];
    failures: WdImageFailure[];
};

export type WdVideoFramePrediction = {
    frameNumber: number;
    timestampSeconds: number;
    tags: WdTagScore[];
};

export type WdTaggerVideoResult = {
    sessionId: string;
    videoPath: string;
    durationSeconds: number;
    frames: WdVideoFramePrediction[];
    tags: WdTagScore[];
};

export type WdTaggerUnloadResult = {
    sessionId: string;
    removed: boolean;
};

export type LlamafileLoadResult = {
    sessionId: string;
    port: number;
    reused: boolean;
};

export type LlamafileProcessImageResult = {
    sessionId: string;
    imagePath: string;
    content: string;
};

export type LlamafileUnloadResult = {
    sessionId: string;
    removed: boolean;
};
export type VideoExtractFramesResult = { videoPath: string; durationSeconds: number; framePaths: string[]; directory: string };
export type VideoCleanupFramesResult = { removed: boolean };
export type RemoteVisionProcessImageResult = { provider: RemoteVisionProvider; model: string; imagePath: string; content: string };
export type RemoteVisionBatchItemResult = { itemId: string; imagePath: string; content: string; error: string };
export type RemoteVisionProcessBatchResult = { provider: RemoteVisionProvider; model: string; results: RemoteVisionBatchItemResult[] };

export type ResultDataMap = {
    echo: EchoResult;
    check_for_update: CheckForUpdateResult;
    download_file: DownloadFileResult;
    read_config: ReadConfigResult;
    write_config: WriteConfigResult;
    path_info: PathInfoResult;
    backup_write: BackupWriteResult;
    backup_list: BackupListResult;
    backup_read: BackupReadResult;
    scan_wd_models: ScanWdModelsResult;
    scan_embedding_models: ScanEmbeddingModelsResult;
    embedding_load: EmbeddingLoadResult;
    embedding_index_batch: EmbeddingIndexBatchResult;
    embedding_index_tags: EmbeddingIndexTagsResult;
    embedding_index_annotations: EmbeddingIndexAnnotationsResult;
    embedding_prune: EmbeddingPruneResult;
    embedding_prune_tags: EmbeddingPruneTagsResult;
    embedding_prune_annotations: EmbeddingPruneAnnotationsResult;
    embedding_health: EmbeddingHealthResult;
    embedding_status: EmbeddingStatusResult;
    embedding_migrate_text: EmbeddingMigrateTextResult;
    embedding_search_text: EmbeddingSearchResult;
    embedding_search_image: EmbeddingSearchResult;
    embedding_unload: EmbeddingUnloadResult;
    wd_tagger_load: WdTaggerLoadResult;
    wd_tagger_enqueue: WdTaggerEnqueueResult;
    wd_tagger_batch_complete: WdTaggerBatchCompleteResult;
    wd_tagger_video: WdTaggerVideoResult;
    video_extract_frames: VideoExtractFramesResult;
    video_cleanup_frames: VideoCleanupFramesResult;
    wd_tagger_unload: WdTaggerUnloadResult;
    llamafile_load: LlamafileLoadResult;
    llamafile_process_image: LlamafileProcessImageResult;
    llamafile_unload: LlamafileUnloadResult;
    remote_vision_process_image: RemoteVisionProcessImageResult;
    remote_vision_process_batch: RemoteVisionProcessBatchResult;
};

export type ResultKind = keyof ResultDataMap;

export type ResultPayload = {
    [K in ResultKind]: {
        kind: K;
        data: ResultDataMap[K];
    };
}[ResultKind];

export type DownloadFileProgress = {
    downloadedBytes: number;
    remainingBytes: number | null;
    totalBytes: number | null;
    bytesPerSecond: number;
    percentage: number | null;
    elapsedMilliseconds: number;
};

export type ProgressDataMap = {
    download_file: DownloadFileProgress;
    remote_vision_batch_item: RemoteVisionBatchItemResult;
    embedding_text_migration: {
        phase: string;
        completed: number;
        total: number;
    };
};

export type ProgressKind = keyof ProgressDataMap;

export type ProgressPayload = {
    [K in ProgressKind]: {
        kind: K;
        data: ProgressDataMap[K];
    };
}[ProgressKind];

export type ErrorPayload = {
    code: string;
    message: string;
};

export type WorkerResultMessage = {
    protocolVersion: number;
    requestId: string;
    type: "result";
    payload: ResultPayload;
};

export type WorkerProgressMessage = {
    protocolVersion: number;
    requestId: string;
    type: "progress";
    payload: ProgressPayload;
};

export type WorkerErrorMessage = {
    protocolVersion: number;
    requestId?: string;
    type: "error";
    payload: ErrorPayload;
};

export type WorkerMessage =
    | WorkerResultMessage
    | WorkerProgressMessage
    | WorkerErrorMessage;

export function createRequest<K extends CommandType>(
    requestId: string,
    type: K,
    payload: CommandPayloadMap[K],
): WorkerRequest<K> {
    return {
        protocolVersion: PROTOCOL_VERSION,
        requestId,
        type,
        payload,
    } as WorkerRequest<K>;
}
