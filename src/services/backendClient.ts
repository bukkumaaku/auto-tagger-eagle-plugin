import {
    type CheckForUpdateResult,
    type CommandPayloadMap,
    type CommandType,
    type Config,
    type DownloadFileProgress,
    type DownloadFileResult,
    type EchoResult,
    type ReadConfigResult,
    type PathInfoResult,
    type BackupListResult,
    type BackupReadResult,
    type BackupWriteResult,
    type ScanWdModelsResult,
    type ProgressPayload,
    PROTOCOL_VERSION,
    type ResultDataMap,
    type WorkerMessage,
    type WriteConfigResult,
    type WdExecutionProvider,
    type WdModelKind,
    type WdTaggerImage,
    type WdTagLanguage,
    type WdTaggerLoadResult,
    type WdTaggerBatchCompleteResult,
    type WdTaggerEnqueueResult,
    type WdTaggerUnloadResult,
    type WdTaggerVideoResult,
    type LlamafileLoadRequest,
    type LlamafileLoadResult,
    type LlamafileProcessImageRequest,
    type LlamafileProcessImageResult,
    type LlamafileUnloadResult,
    type EmbeddingExecutionProvider,
    type EmbeddingProvider,
    type EmbeddingImageInput,
    type EmbeddingIndexBatchResult,
    type EmbeddingTagInput,
    type EmbeddingIndexTagsResult,
    type EmbeddingPruneTagsResult,
    type EmbeddingHealthResult,
    type EmbeddingPruneResult,
    type EmbeddingLoadResult,
    type EmbeddingModelInfo,
    type EmbeddingSearchResult,
    type EmbeddingStatusResult,
    type EmbeddingMigrateTextResult,
    type EmbeddingUnloadResult,
    createRequest,
} from "../protocol";

type PendingRequest = {
    expectedKind: keyof ResultDataMap;
    resolve: (value: unknown) => void;
    reject: (error: Error) => void;
    onProgress?: (progress: ProgressPayload) => void;
};

export class BackendClientError extends Error {
    readonly code: string;
    readonly requestId?: string;

    constructor(
        code: string,
        message: string,
        requestId?: string,
    ) {
        super(message);
        this.name = "BackendClientError";
        this.code = code;
        this.requestId = requestId;
    }
}

export class BackendClient {
    private readonly workerPath: string;
    private child: any = null;
    private lines: any = null;
    private sequence = 0;
    private generation = 0;
    private readonly pending = new Map<string, PendingRequest>();

    constructor(workerPath: string) {
        this.workerPath = workerPath;
    }

    get path() {
        return this.workerPath;
    }

    get isRunning() {
        return Boolean(this.child && !this.child.killed);
    }

    get workerGeneration() {
        return this.generation;
    }

    start() {
        if (this.isRunning) return;

        const fs = require("fs");
        const { spawn } = require("child_process");
        const readline = require("readline");

        if (!fs.existsSync(this.workerPath)) {
            throw new BackendClientError(
                "WORKER_NOT_FOUND",
                `ai-worker was not found at ${this.workerPath}`,
            );
        }

        this.child = spawn(this.workerPath, [], {
            stdio: ["pipe", "pipe", "pipe"],
            windowsHide: true,
        });
        this.lines = readline.createInterface({ input: this.child.stdout });
        this.lines.on("line", (line: string) => this.handleLine(line));
        this.child.stderr.on("data", (data: { toString(): string }) => {
            console.error(`[ai-worker] ${data.toString().trim()}`);
        });
        this.child.on("error", (error: Error) => this.rejectAll(error));
        this.child.on("exit", (code: number | null) => {
            this.lines?.close();
            this.lines = null;
            this.child = null;
            this.generation++;
            this.rejectAll(
                new BackendClientError(
                    "WORKER_EXITED",
                    `ai-worker exited with code ${code}`,
                ),
            );
        });
    }

    echo(message: string): Promise<EchoResult> {
        return this.request("echo", { message });
    }

    checkForUpdate(): Promise<CheckForUpdateResult> {
        return this.request("check_for_update", {});
    }

    downloadFile(
        url: string,
        destination: string,
        onProgress?: (progress: DownloadFileProgress) => void,
    ): Promise<DownloadFileResult> {
        return this.request("download_file", { url, destination }, (payload) => {
            if (payload.kind === "download_file") {
                onProgress?.(payload.data);
            }
        });
    }

    readConfig(): Promise<ReadConfigResult> {
        return this.request("read_config", {});
    }

    writeConfig(config: Config): Promise<WriteConfigResult> {
        return this.request("write_config", { config });
    }

    pathInfo(path: string): Promise<PathInfoResult> {
        return this.request("path_info", { path });
    }

    writeBackup(directory: string, filename: string, content: string): Promise<BackupWriteResult> {
        return this.request("backup_write", { directory, filename, content });
    }

    listBackups(directory: string): Promise<BackupListResult> {
        return this.request("backup_list", { directory });
    }

    readBackup(path: string): Promise<BackupReadResult> {
        return this.request("backup_read", { path });
    }

    scanWdModels(root: string): Promise<ScanWdModelsResult> {
        return this.request("scan_wd_models", { root });
    }

    async scanEmbeddingModels(root: string): Promise<{
        modelsDirectory: string;
        models: EmbeddingModelInfo[];
    }> {
        return this.request("scan_embedding_models", { root });
    }

    loadEmbedding(
        sessionId: string,
        modelKey: string,
        modelPath: string,
        tokenizerPath: string,
        databasePath: string,
        namespace: string,
        executionProvider: EmbeddingExecutionProvider = "auto",
        provider: EmbeddingProvider = "local",
        endpoint = "",
        apiKey = "",
        remoteModel = "",
        remoteDimension = 0,
        legacyModelKey = "",
    ): Promise<EmbeddingLoadResult> {
        return this.request("embedding_load", {
            sessionId,
            modelKey,
            modelPath,
            tokenizerPath,
            databasePath,
            namespace,
            executionProvider,
            provider,
            endpoint,
            apiKey,
            remoteModel,
            remoteDimension,
            legacyModelKey,
        });
    }

    indexEmbeddingBatch(
        sessionId: string,
        images: EmbeddingImageInput[],
        force = false,
    ): Promise<EmbeddingIndexBatchResult> {
        return this.request("embedding_index_batch", { sessionId, images, force });
    }

    indexEmbeddingTags(
        sessionId: string,
        items: EmbeddingTagInput[],
        concurrency: number,
        force = false,
        forceTagIds: string[] = [],
    ): Promise<EmbeddingIndexTagsResult> {
        return this.request("embedding_index_tags", {
            sessionId,
            items,
            concurrency,
            force,
            forceTagIds,
        });
    }
    indexEmbeddingAnnotations(sessionId: string, items: import("../protocol").EmbeddingAnnotationInput[], concurrency: number, force = false): Promise<import("../protocol").EmbeddingIndexAnnotationsResult> { return this.request("embedding_index_annotations", { sessionId, items, concurrency, force }); }

    pruneEmbeddingTags(sessionId: string, itemIds: string[]): Promise<EmbeddingPruneTagsResult> {
        return this.request("embedding_prune_tags", { sessionId, itemIds });
    }
    pruneEmbeddingAnnotations(sessionId: string, itemIds: string[]): Promise<import("../protocol").EmbeddingPruneAnnotationsResult> { return this.request("embedding_prune_annotations", { sessionId, itemIds }); }

    pruneEmbedding(
        sessionId: string,
        itemIds: string[],
    ): Promise<EmbeddingPruneResult> {
        return this.request("embedding_prune", { sessionId, itemIds });
    }

    embeddingHealth(
        sessionId: string,
        itemIds: string[],
    ): Promise<EmbeddingHealthResult> {
        return this.request("embedding_health", { sessionId, itemIds });
    }

    embeddingStatus(
        sessionId: string,
        options: {
            databasePath?: string;
            namespace?: string;
            modelKey?: string;
            dimension?: number;
            legacyModelKey?: string;
        } = {},
    ): Promise<EmbeddingStatusResult> {
        return this.request("embedding_status", { sessionId, ...options });
    }

    migrateEmbeddingText(
        databasePath: string,
        namespace: string,
        modelKey: string,
        dimension: number,
        legacyModelKey = "",
        onProgress?: (progress: {
            phase: string;
            completed: number;
            total: number;
        }) => void,
    ): Promise<EmbeddingMigrateTextResult> {
        return this.request(
            "embedding_migrate_text",
            { databasePath, namespace, modelKey, dimension, legacyModelKey },
            (progress) => {
                if (progress.kind === "embedding_text_migration") {
                    onProgress?.(progress.data);
                }
            },
        );
    }

    searchEmbeddingText(
        sessionId: string,
        text: string,
        topK: number,
        includeImage = true,
        includeTags = false,
        includeAnnotations = false,
    ): Promise<EmbeddingSearchResult> {
        return this.request("embedding_search_text", {
            sessionId,
            text,
            topK,
            includeImage,
            includeTags,
            includeAnnotations,
        });
    }

    searchEmbeddingImage(
        sessionId: string,
        imagePath: string,
        excludeItemId: string,
        imageModifiedAt: number,
        topK: number,
    ): Promise<EmbeddingSearchResult> {
        return this.request("embedding_search_image", {
            sessionId,
            imagePath,
            excludeItemId,
            imageModifiedAt,
            topK,
        });
    }

    unloadEmbedding(sessionId: string): Promise<EmbeddingUnloadResult> {
        return this.request("embedding_unload", { sessionId });
    }

    loadWdTagger(
        sessionId: string,
        modelPath: string,
        tagsPath: string,
        modelKind: WdModelKind = "wd",
        executionProvider: WdExecutionProvider = "auto",
        tagsetPath = "",
        language: WdTagLanguage = "en",
        splitter = "",
        filterTags: string[] = [],
    ): Promise<WdTaggerLoadResult> {
        return this.request("wd_tagger_load", {
            sessionId,
            modelPath,
            tagsPath,
            modelKind,
            executionProvider,
            tagsetPath,
            language,
            splitter,
            filterTags,
        });
    }

    enqueueWdTaggerImage(
        sessionId: string,
        image: WdTaggerImage,
    ): Promise<WdTaggerEnqueueResult> {
        return this.request("wd_tagger_enqueue", {
            sessionId,
            image,
        });
    }

    completeWdTaggerBatch(
        sessionId: string,
        threshold = 0.25,
    ): Promise<WdTaggerBatchCompleteResult> {
        return this.request("wd_tagger_batch_complete", {
            sessionId,
            threshold,
        });
    }

    tagVideoWithWdTagger(
        sessionId: string,
        videoPath: string,
        ffmpegPath: string,
        ffprobePath: string,
        frameCount: number,
        batchSize: number,
        threshold = 0.25,
    ): Promise<WdTaggerVideoResult> {
        return this.request("wd_tagger_video", {
            sessionId,
            videoPath,
            ffmpegPath,
            ffprobePath,
            frameCount,
            batchSize,
            threshold,
        });
    }

    unloadWdTagger(sessionId: string): Promise<WdTaggerUnloadResult> {
        return this.request("wd_tagger_unload", { sessionId });
    }

    loadLlamafile(request: LlamafileLoadRequest): Promise<LlamafileLoadResult> {
        return this.request("llamafile_load", request);
    }

    processImageWithLlamafile(
        request: LlamafileProcessImageRequest,
    ): Promise<LlamafileProcessImageResult> {
        return this.request("llamafile_process_image", request);
    }

    unloadLlamafile(sessionId: string): Promise<LlamafileUnloadResult> {
        return this.request("llamafile_unload", { sessionId });
    }

    extractVideoFrames(videoPath: string, ffmpegPath: string, ffprobePath: string) {
        return this.request("video_extract_frames", { videoPath, ffmpegPath, ffprobePath });
    }

    cleanupVideoFrames(directory: string) {
        return this.request("video_cleanup_frames", { directory });
    }
    processImageWithRemoteVision(request: import("../protocol").RemoteVisionProcessImageRequest): Promise<import("../protocol").RemoteVisionProcessImageResult> { return this.request("remote_vision_process_image", request); }
    processBatchWithRemoteVision(
        request: import("../protocol").RemoteVisionProcessBatchRequest,
        onItem?: (result: import("../protocol").RemoteVisionBatchItemResult) => void,
    ): Promise<import("../protocol").RemoteVisionProcessBatchResult> {
        return this.request(
            "remote_vision_process_batch",
            request,
            (progress) => {
                if (progress.kind === "remote_vision_batch_item") {
                    onItem?.(progress.data);
                }
            },
        );
    }

    dispose() {
        this.lines?.close();
        this.lines = null;
        this.child?.kill();
        this.child = null;
        this.rejectAll(
            new BackendClientError("WORKER_DISPOSED", "ai-worker was disposed"),
        );
    }

    private request<K extends CommandType>(
        type: K,
        payload: CommandPayloadMap[K],
        onProgress?: (progress: ProgressPayload) => void,
    ): Promise<ResultDataMap[K]> {
        this.start();
        const requestId = `r-${Date.now()}-${++this.sequence}`;
        const request = createRequest(requestId, type, payload);

        return new Promise<ResultDataMap[K]>((resolve, reject) => {
            const pending: PendingRequest = {
                expectedKind: type,
                resolve: (value) => resolve(value as ResultDataMap[K]),
                reject,
                onProgress,
            };
            this.pending.set(requestId, pending);
            const failWrite = (error: Error) => {
                if (!this.pending.delete(requestId)) return;
                pending.reject(error);
            };
            try {
                this.child.stdin.write(
                    `${JSON.stringify(request)}\n`,
                    (error?: Error | null) => {
                        if (error) failWrite(error);
                    },
                );
            } catch (error) {
                failWrite(error instanceof Error ? error : new Error(String(error)));
            }
        });
    }

    private handleLine(line: string) {
        let message: WorkerMessage;
        try {
            message = JSON.parse(line) as WorkerMessage;
        } catch (error) {
            console.error("Invalid JSONL from ai-worker", line, error);
            return;
        }

        if (message.protocolVersion !== PROTOCOL_VERSION) {
            console.error("Unsupported ai-worker protocol version", message);
            return;
        }

        if (!message.requestId) {
            console.error("Uncorrelated ai-worker message", message);
            return;
        }

        const pending = this.pending.get(message.requestId);
        if (!pending) return;

        if (message.type === "progress") {
            try {
                pending.onProgress?.(message.payload);
            } catch (error) {
                console.error("Download progress callback failed", error);
            }
            return;
        }

        this.pending.delete(message.requestId);
        if (message.type === "error") {
            pending.reject(
                new BackendClientError(
                    message.payload.code,
                    message.payload.message,
                    message.requestId,
                ),
            );
            return;
        }

        if (message.payload.kind !== pending.expectedKind) {
            pending.reject(
                new BackendClientError(
                    "RESULT_KIND_MISMATCH",
                    `expected ${pending.expectedKind}, got ${message.payload.kind}`,
                    message.requestId,
                ),
            );
            return;
        }

        pending.resolve(message.payload.data);
    }

    private rejectAll(error: Error) {
        for (const pending of this.pending.values()) {
            pending.reject(error);
        }
        this.pending.clear();
    }
}

let defaultClient: BackendClient | null = null;

export function resolveBundledWorkerPath() {
    const os = require("os");
    const path = require("path");
    const platform = os.platform();
    const architecture = os.arch();
    const workerTargets: Record<string, { directory: string; executable: string }> = {
        "win32-x64": { directory: "win-x64", executable: "ai-worker.exe" },
        "darwin-arm64": { directory: "mac-arm64", executable: "ai-worker" },
    };
    const worker = workerTargets[`${platform}-${architecture}`];

    if (!worker) {
        throw new BackendClientError(
            "UNSUPPORTED_WORKER_PLATFORM",
            `no bundled ai-worker for ${platform}-${architecture}`,
        );
    }

    if (import.meta.env.DEV) {
        const projectRoot = window.pluginPath
            ? path.join(window.pluginPath, "..")
            : path.resolve(".");
        return path.join(
            projectRoot,
            "bin",
            worker.directory,
            worker.executable,
        );
    }

    const pluginDirectory = resolveProductionPluginDirectory(path);

    return path.join(
        pluginDirectory,
        "bin",
        worker.directory,
        worker.executable,
    );
}

function resolveProductionPluginDirectory(path: any): string {
    if (window.pluginPath) {
        return window.pluginPath;
    }

    if (window.location.protocol === "file:") {
        const { fileURLToPath } = require("url");
        return path.dirname(fileURLToPath(window.location.href));
    }

    if (window.location.protocol === "eagleplugin:") {
        const location = new URL(window.location.href);
        const pathname = decodeURIComponent(location.pathname).replace(
            /^\/+([A-Za-z]:[\\/])/,
            "$1",
        );
        if (path.win32.isAbsolute(pathname)) {
            return path.win32.dirname(path.win32.normalize(pathname));
        }
        if (path.posix.isAbsolute(pathname)) {
            return path.posix.dirname(path.posix.normalize(pathname));
        }
    }

    throw new BackendClientError(
        "PLUGIN_PATH_MISSING",
        `window.pluginPath is not available and cannot be inferred from ${window.location.href}`,
    );
}

export function getBackendClient() {
    defaultClient ??= new BackendClient(resolveBundledWorkerPath());
    return defaultClient;
}
