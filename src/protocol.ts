export const PROTOCOL_VERSION = 1 as const;

export type EchoRequest = {
    message: string;
};

export type AaaRequest = {
    value: string;
};

export type CheckForUpdateRequest = Record<string, never>;

export type DownloadFileRequest = {
    url: string;
    destination: string;
};

export type Config = {
    [key: string]: string | number | boolean | string[];
    endpoint: string;
    apiKey: string;
    embeddingProvider: EmbeddingProvider;
    embeddingDimension: number;
    modelPath: string;
    threshold: number;
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
};

export type EmbeddingImageInput = {
    id: string;
    path: string;
    name: string;
    modifiedAt: number;
};

export type EmbeddingIndexBatchRequest = {
    sessionId: string;
    images: EmbeddingImageInput[];
};

export type EmbeddingPruneRequest = {
    sessionId: string;
    itemIds: string[];
};

export type EmbeddingHealthRequest = {
    sessionId: string;
    itemIds: string[];
};

export type EmbeddingStatusRequest = {
    sessionId: string;
};

export type EmbeddingSearchTextRequest = {
    sessionId: string;
    text: string;
    topK: number;
};

export type EmbeddingSearchImageRequest = {
    sessionId: string;
    imagePath: string;
    excludeItemId: string;
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

export type CommandPayloadMap = {
    echo: EchoRequest;
    aaa: AaaRequest;
    check_for_update: CheckForUpdateRequest;
    download_file: DownloadFileRequest;
    read_config: ReadConfigRequest;
    write_config: WriteConfigRequest;
    path_info: PathInfoRequest;
    scan_wd_models: ScanWdModelsRequest;
    scan_embedding_models: ScanEmbeddingModelsRequest;
    embedding_load: EmbeddingLoadRequest;
    embedding_index_batch: EmbeddingIndexBatchRequest;
    embedding_prune: EmbeddingPruneRequest;
    embedding_health: EmbeddingHealthRequest;
    embedding_status: EmbeddingStatusRequest;
    embedding_search_text: EmbeddingSearchTextRequest;
    embedding_search_image: EmbeddingSearchImageRequest;
    embedding_unload: EmbeddingUnloadRequest;
    wd_tagger_load: WdTaggerLoadRequest;
    wd_tagger_enqueue: WdTaggerEnqueueRequest;
    wd_tagger_batch_complete: WdTaggerBatchCompleteRequest;
    wd_tagger_video: WdTaggerVideoRequest;
    wd_tagger_unload: WdTaggerUnloadRequest;
    llamafile_load: LlamafileLoadRequest;
    llamafile_process_image: LlamafileProcessImageRequest;
    llamafile_unload: LlamafileUnloadRequest;
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

export type AaaResult = {
    value: string;
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
    indexedCount: number;
    reused: boolean;
};

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

export type EmbeddingPruneResult = {
    sessionId: string;
    removedCount: number;
    totalIndexed: number;
};

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
};

export type EmbeddingStatusResult = {
    sessionId: string;
    modelKey: string;
    indexedCount: number;
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

export type ResultDataMap = {
    echo: EchoResult;
    aaa: AaaResult;
    check_for_update: CheckForUpdateResult;
    download_file: DownloadFileResult;
    read_config: ReadConfigResult;
    write_config: WriteConfigResult;
    path_info: PathInfoResult;
    scan_wd_models: ScanWdModelsResult;
    scan_embedding_models: ScanEmbeddingModelsResult;
    embedding_load: EmbeddingLoadResult;
    embedding_index_batch: EmbeddingIndexBatchResult;
    embedding_prune: EmbeddingPruneResult;
    embedding_health: EmbeddingHealthResult;
    embedding_status: EmbeddingStatusResult;
    embedding_search_text: EmbeddingSearchResult;
    embedding_search_image: EmbeddingSearchResult;
    embedding_unload: EmbeddingUnloadResult;
    wd_tagger_load: WdTaggerLoadResult;
    wd_tagger_enqueue: WdTaggerEnqueueResult;
    wd_tagger_batch_complete: WdTaggerBatchCompleteResult;
    wd_tagger_video: WdTaggerVideoResult;
    wd_tagger_unload: WdTaggerUnloadResult;
    llamafile_load: LlamafileLoadResult;
    llamafile_process_image: LlamafileProcessImageResult;
    llamafile_unload: LlamafileUnloadResult;
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
