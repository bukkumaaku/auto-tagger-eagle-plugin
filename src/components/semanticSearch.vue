<script setup lang="ts">
    import {
        NAlert,
        NButton,
        NCheckbox,
        NEmpty,
        NIcon,
        NInput,
        NInputNumber,
        NProgress,
        NRadioButton,
        NRadioGroup,
        NSelect,
        NSlider,
        NSpace,
        NSpin,
        NTabPane,
        NTabs,
        NTag,
        NTooltip,
    } from "naive-ui";
    import {
        CloudDownloadOutline,
        ImageOutline,
        InformationCircleOutline,
        OpenOutline,
        PlayOutline,
        PulseOutline,
        PricetagsOutline,
        ReloadOutline,
        SearchOutline,
        TrashOutline,
    } from "@vicons/ionicons5";
    import {
        computed,
        nextTick,
        onBeforeUnmount,
        onMounted,
        ref,
        watch,
    } from "vue";
    import type {
        EmbeddingImageFailure,
        EmbeddingImageInput,
        EmbeddingAnnotationInput,
        EmbeddingTagInput,
        EmbeddingHealthResult,
        EmbeddingModelInfo,
        EmbeddingSearchHit,
    } from "../protocol";
    import { backenAPI, config, configRevision, dialog, notification, t } from "../api/backen";
    import { getBackendClient } from "../services/backendClient";
    import {
        beginTask,
        cancelTask,
        completeTask,
        failTask,
        isTaskCancelled,
        recordFailure,
        updateTask,
        waitForTaskControl,
    } from "../services/taskCenter";
    import downloadModal from "./downloadModal.vue";
    import FormHelp from "./formHelp.vue";
    import TaskControlButtons from "./taskControlButtons.vue";

    const SESSION_ID = "embedding-main";
    const INDEX_FILENAME = "semantic-index.sqlite3";
    const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "webp", "bmp"]);
    const path = require("node:path");
    const crypto = require("node:crypto");
    const { pathToFileURL } = require("node:url");

    type EagleImage = {
        id: string;
        name?: string;
        ext?: string;
        filePath?: string;
        thumbnailPath?: string;
        modifiedAt?: number;
        width?: number;
        height?: number;
        isDeleted?: boolean;
        tags?: string[];
        annotation?: string;
    };

    type SearchDisplayItem = EmbeddingSearchHit & {
        thumbnailUrl: string;
        previewUrl: string;
        displayName: string;
        aspectRatio: string;
    };

    type SelectableEmbeddingModel = EmbeddingModelInfo & {
        provider: "local" | "open_ai" | "gemini";
        remoteModel: string;
        selectionKey: string;
        endpoint: string;
        apiKey: string;
        legacyModelKey: string;
    };

    const activeTab = ref("index");
    const MAX_GEMINI_CONCURRENCY = 50;
    const MAX_OPENAI_CONCURRENCY = 16;
    const MAX_LOCAL_BATCH_SIZE = 32;
    const SEARCH_RESULT_PAGE_SIZE = 60;
    const MAX_SEARCH_RESULTS = 4_096;
    const models = ref<SelectableEmbeddingModel[]>([]);
    const selectedModel = ref("");
    const batchSize = ref(8);
    const isReady = ref(false);
    const isLoadingModels = ref(false);
    const showDownload = ref(false);
    const loadedSignature = ref("");
    const loadedModelKey = ref("");
    const indexedCount = ref(0);
    const tagDocumentCount = ref(0);
    const tagIndexedCount = ref(0);
    const tagLinkCount = ref(0);
    const isTagIndexing = ref(false);
    const tagIndexStatus = ref("就绪");
    const tagTotalItems = ref(0);
    const tagProcessedItems = ref(0);
    const tagIndexFailures = ref<string[]>([]);
    const annotationDocumentCount = ref(0);
    const annotationIndexedCount = ref(0);
    const libraryAnnotationCount = ref(0);
    const isAnnotationIndexing = ref(false);
    const annotationIndexStatus = ref("就绪");
    const annotationTotalItems = ref(0);
    const annotationProcessedItems = ref(0);
    const annotationIndexFailures = ref<string[]>([]);

    const isIndexing = ref(false);
    const totalImages = ref(0);
    const libraryTagCount = ref(0);
    const libraryCountsReady = ref(false);
    const libraryReadError = ref("");
    const processedImages = ref(0);
    const indexedThisRun = ref(0);
    const skippedThisRun = ref(0);
    const indexFailures = ref<EmbeddingImageFailure[]>([]);
    const indexStatus = ref("就绪");
    const activeSemanticTaskId = ref("");
    let disposed = false;

    const searchMode = ref<"text" | "image">("text");
    const includeImages = ref(true);
    const includeTags = ref(false);
    const includeAnnotations = ref(false);
    const queryText = ref("");
    const negativeQueryText = ref("");
    const negativePromptWeight = ref(0.3);
    const isSearching = ref(false);
    const allSearchHits = ref<EmbeddingSearchHit[]>([]);
    const searchResults = ref<SearchDisplayItem[]>([]);
    const nextSearchHitOffset = ref(0);
    const isLoadingMoreResults = ref(false);
    const searchLoadSentinel = ref<HTMLElement | null>(null);
    const masonryGrid = ref<HTMLElement | null>(null);
    const expandedResult = ref<SearchDisplayItem | null>(null);
    const healthResult = ref<EmbeddingHealthResult | null>(null);
    const isCheckingHealth = ref(false);
    let searchResultObserver: IntersectionObserver | null = null;
    let masonryResizeObserver: ResizeObserver | null = null;
    let masonryGridWidth = 0;
    let searchGeneration = 0;
    let previewClickTimer: ReturnType<typeof setTimeout> | undefined;
    let settingsSaveTimer: ReturnType<typeof setTimeout> | undefined;

    const modelOptions = computed(() =>
        models.value.map((model) => ({
            label: model.name,
            value: model.selectionKey,
        })),
    );
    const selectedModelInfo = computed(() =>
        models.value.find((model) => model.selectionKey === selectedModel.value),
    );
    const isRemoteModel = computed(
        () =>
            selectedModelInfo.value?.provider === "open_ai" ||
            selectedModelInfo.value?.provider === "gemini",
    );
    const batchFieldLabel = computed(() =>
        isRemoteModel.value ? "并发" : "批次",
    );
    const batchFieldMax = computed(() => {
        if (selectedModelInfo.value?.provider === "gemini")
            return MAX_GEMINI_CONCURRENCY;
        if (selectedModelInfo.value?.provider === "open_ai")
            return MAX_OPENAI_CONCURRENCY;
        return MAX_LOCAL_BATCH_SIZE;
    });
    const remoteModelKey = (
        provider: "open_ai" | "gemini",
        model: string,
        dimension: number,
    ) => {
        const identity = [provider, model.trim(), dimension].join("\u0000");
        const digest = crypto
            .createHash("sha256")
            .update(identity, "utf8")
            .digest("hex")
            .slice(0, 16);
        return `${provider}:${model}:${dimension || "auto"}:${digest}`;
    };
    const endpointRemoteModelKey = (
        provider: "open_ai" | "gemini",
        endpoint: string,
        model: string,
        dimension: number,
    ) => {
        const identity = [
            provider,
            endpoint.trim().replace(/\/+$/, ""),
            model.trim(),
            dimension,
        ].join("\u0000");
        const digest = crypto
            .createHash("sha256")
            .update(identity, "utf8")
            .digest("hex")
            .slice(0, 16);
        return `${provider}:${model}:${dimension || "auto"}:${digest}`;
    };
    const indexPercentage = computed(() =>
        totalImages.value === 0
            ? 0
            : Number(
                  ((processedImages.value / totalImages.value) * 100).toFixed(
                      2,
                  ),
              ),
    );
    const hasMoreSearchResults = computed(
        () => nextSearchHitOffset.value < allSearchHits.value.length,
    );
    const isSearchResultCapped = computed(
        () => indexedCount.value > MAX_SEARCH_RESULTS,
    );
    const imagePendingCount = computed(() =>
        Math.max(totalImages.value - indexedCount.value, 0),
    );
    const tagTotalCount = computed(() =>
        libraryCountsReady.value ? libraryTagCount.value : tagDocumentCount.value,
    );
    const tagPendingCount = computed(() =>
        Math.max(tagTotalCount.value - tagIndexedCount.value, 0),
    );
    const canIncludeTags = computed(
        () => tagIndexedCount.value > 0 && tagLinkCount.value > 0,
    );
    const canIncludeAnnotations = computed(() => annotationIndexedCount.value > 0);
    const canIncludeImages = computed(() => indexedCount.value > 0);
    const annotationPendingCount = computed(() => Math.max(libraryAnnotationCount.value - annotationIndexedCount.value, 0));
    const annotationIndexPercentage = computed(() => annotationTotalItems.value === 0 ? 0 : Number(((annotationProcessedItems.value / annotationTotalItems.value) * 100).toFixed(2)));
    const tagIndexPercentage = computed(() =>
        tagTotalItems.value === 0
            ? 0
            : Number(
                  ((tagProcessedItems.value / tagTotalItems.value) * 100).toFixed(2),
              ),
    );

    onMounted(async () => {
        try {
            await backenAPI.getConfig();
            negativePromptWeight.value = normalizeNegativePromptWeight(
                config.negativePromptWeight,
            );
            batchSize.value = Math.min(
                MAX_GEMINI_CONCURRENCY,
                Math.max(1, Number(config.embeddingBatchSize) || 8),
            );
            selectedModel.value = config.embeddingModelId || "";
            config.embeddingBatchSize = batchSize.value;
            config.embeddingDevice = config.embeddingDevice || "auto";
            config.embeddingProvider =
                config.embeddingProvider === "gemini" ? "gemini" : "open_ai";
            config.embeddingDimension = Math.min(
                3072,
                Math.max(128, Number(config.embeddingDimension) || 1536),
            );
            await refreshModels();
            try {
                await refreshLibraryCounts(true);
            } catch (error) {
                notification(errorMessage(error), "error");
            }
            await refreshIndexStatus();
            await persistSettings();
            isReady.value = true;
        } catch (error) {
            notification(errorMessage(error), "error");
        }
    });

    onBeforeUnmount(() => {
        disposed = true;
        searchGeneration += 1;
        if (previewClickTimer) clearTimeout(previewClickTimer);
        if (settingsSaveTimer) clearTimeout(settingsSaveTimer);
        settingsSaveTimer = undefined;
        if (isReady.value) void persistSettings();
        searchResultObserver?.disconnect();
        masonryResizeObserver?.disconnect();
        void getBackendClient().unloadEmbedding(SESSION_ID).catch(() => {});
    });

    watch([selectedModel, batchSize, negativePromptWeight], () => {
        if (isReady.value) schedulePersistSettings();
    });
    watch(selectedModel, () => {
        if (isReady.value) void refreshIndexStatus();
    });
    watch(batchFieldMax, (maximum) => {
        if (batchSize.value > maximum) batchSize.value = maximum;
    });
    watch(configRevision, () => {
        if (isReady.value) void refreshModels();
    });
    watch(canIncludeTags, (enabled) => {
        if (!enabled) includeTags.value = false;
    });
    watch(canIncludeAnnotations, (enabled) => { if (!enabled) includeAnnotations.value = false; });
    watch(canIncludeImages, (enabled) => { if (!enabled) includeImages.value = false; });
    watch(searchMode, (mode) => {
        if (mode !== "text") { includeImages.value = true; includeTags.value = false; includeAnnotations.value = false; }
    });
    watch(searchLoadSentinel, (sentinel) => {
        searchResultObserver?.disconnect();
        searchResultObserver = null;
        if (!sentinel) return;
        searchResultObserver = new IntersectionObserver(
            (entries) => {
                if (entries.some((entry) => entry.isIntersecting)) {
                    void loadMoreSearchResults();
                }
            },
            { rootMargin: "800px 0px" },
        );
        searchResultObserver.observe(sentinel);
    });
    watch(masonryGrid, (grid) => {
        masonryResizeObserver?.disconnect();
        masonryResizeObserver = null;
        masonryGridWidth = 0;
        if (!grid) return;
        masonryResizeObserver = new ResizeObserver(([entry]) => {
            const width = entry?.contentRect.width || 0;
            if (Math.abs(width - masonryGridWidth) < 0.5) return;
            masonryGridWidth = width;
            refreshMasonryLayout();
        });
        masonryResizeObserver.observe(grid);
        refreshMasonryLayout();
    });

    async function persistSettings() {
        config.embeddingModelId = selectedModel.value;
        config.embeddingBatchSize = batchSize.value;
        config.negativePromptWeight = normalizeNegativePromptWeight(
            negativePromptWeight.value,
        );
        await backenAPI.setConfig();
    }

    function schedulePersistSettings() {
        if (settingsSaveTimer) clearTimeout(settingsSaveTimer);
        settingsSaveTimer = setTimeout(() => {
            settingsSaveTimer = undefined;
            void persistSettings();
        }, 200);
    }

    function normalizeNegativePromptWeight(value: unknown) {
        const numeric = Number(value);
        return Number.isFinite(numeric)
            ? Math.min(1, Math.max(0, numeric))
            : 0.3;
    }

    async function refreshModels() {
        isLoadingModels.value = true;
        try {
            const localModels = config.modelLocation
                ? (
                      await getBackendClient().scanEmbeddingModels(
                          config.modelLocation,
                      )
                  ).models
                : [];
            const profiles = config.embeddingRemoteProfiles?.length ? config.embeddingRemoteProfiles : (config.embeddingModelName && config.endpoint ? [{ id: "legacy", name: "远程接口", provider: config.embeddingProvider === "gemini" ? "gemini" as const : "open_ai" as const, endpoint: config.endpoint, apiKey: config.apiKey, model: config.embeddingModelName, dimension: config.embeddingDimension }] : []);
            models.value = [
                ...localModels.map((model) => ({
                    ...model,
                    provider: "local" as const,
                    remoteModel: "",
                    selectionKey: model.modelKey,
                    endpoint: "",
                    apiKey: "",
                    legacyModelKey: "",
                })),
                ...profiles.filter((profile) => profile.model && profile.endpoint).map((profile) => ({ name: `${profile.name || profile.model} · ${profile.model}`, modelKey: remoteModelKey(profile.provider, profile.model, profile.provider === "gemini" ? profile.dimension : 0), modelPath: "", tokenizerPath: "", dimension: profile.provider === "gemini" ? profile.dimension : 0, provider: profile.provider, remoteModel: profile.model, selectionKey: `remote:${profile.id}`, endpoint: profile.endpoint, apiKey: profile.apiKey, legacyModelKey: endpointRemoteModelKey(profile.provider, profile.endpoint, profile.model, profile.provider === "gemini" ? profile.dimension : 0) })),
            ];
            if (
                !models.value.some(
                    (model) => model.selectionKey === selectedModel.value,
                )
            ) {
                selectedModel.value = models.value[0]?.selectionKey || "";
            }
        } finally {
            isLoadingModels.value = false;
        }
    }

    async function refreshIndexStatus() {
        const model = selectedModelInfo.value;
        if (!model || !config.modelLocation) return;
        const databasePath = path.join(
            config.modelLocation,
            "embedding",
            INDEX_FILENAME,
        );
        try {
            const namespace = await resolveLibraryNamespace();
            const status = await getBackendClient().embeddingStatus(SESSION_ID, {
                databasePath,
                namespace,
                modelKey: model.modelKey,
                dimension: model.dimension,
                legacyModelKey: model.legacyModelKey,
            });
            indexedCount.value = status.indexedCount;
            tagDocumentCount.value = status.tagDocumentCount;
            tagIndexedCount.value = status.tagIndexedCount;
            tagLinkCount.value = status.tagLinkCount;
            annotationDocumentCount.value = status.annotationDocumentCount;
            annotationIndexedCount.value = status.annotationIndexedCount;
        } catch (error) {
            indexStatus.value = `索引状态读取失败：${errorMessage(error)}`;
            console.error("读取语义索引状态失败", error);
        }
    }

    async function ensureSession() {
        const backend = getBackendClient();
        const model = selectedModelInfo.value;
        if (!model) throw new Error("请先下载并选择向量模型");
        if (!config.modelLocation) throw new Error("请先设置模型根目录");
        if (model.provider !== "local" && !model.endpoint.trim()) {
            throw new Error("请先设置远程向量接口");
        }
        const databasePath = path.join(
            config.modelLocation,
            "embedding",
            INDEX_FILENAME,
        );
        const namespace = await resolveLibraryNamespace();
        const signature = [
            model.modelKey,
            model.modelPath,
            databasePath,
            namespace,
            config.embeddingDevice || "auto",
            model.provider,
            model.remoteModel,
            model.endpoint,
            model.apiKey,
            model.dimension,
            backend.workerGeneration,
        ].join("\u0000");
        if (loadedSignature.value && loadedSignature.value !== signature) {
            await backend.unloadEmbedding(SESSION_ID);
            loadedSignature.value = "";
            loadedModelKey.value = "";
        }
        if (loadedSignature.value !== signature) {
            indexStatus.value = "正在加载模型";
            const result = await backend.loadEmbedding(
                SESSION_ID,
                model.modelKey,
                model.modelPath,
                model.tokenizerPath,
                databasePath,
                namespace,
                config.embeddingDevice === "cpu"
                    ? "cpu"
                    : config.embeddingDevice === "direct_ml"
                      ? "direct_ml"
                      : "auto",
                model.provider,
                model.endpoint,
                model.apiKey,
                model.remoteModel,
                model.provider === "gemini" ? model.dimension : 0,
                model.legacyModelKey,
            );
            indexedCount.value = result.indexedCount;
            tagDocumentCount.value = result.tagDocumentCount;
            tagIndexedCount.value = result.tagIndexedCount;
            tagLinkCount.value = result.tagLinkCount;
            annotationDocumentCount.value = result.annotationDocumentCount;
            annotationIndexedCount.value = result.annotationIndexedCount;
            loadedSignature.value = signature;
            loadedModelKey.value = model.selectionKey;
        }
    }

    async function resolveLibraryNamespace(): Promise<string> {
        let info: Record<string, unknown> | undefined;
        let infoError: unknown;
        if (typeof eagle.library?.info === "function") {
            try {
                const result = await eagle.library.info();
                if (result && typeof result === "object") {
                    info = result as Record<string, unknown>;
                }
            } catch (error) {
                infoError = error;
                console.warn("eagle.library.info() 调用失败，尝试旧版属性", error);
            }
        }

        const candidates = [
            info?.path,
            info?.libraryPath,
            eagle.library?.path,
            info?.name,
            eagle.library?.name,
        ];
        for (const candidate of candidates) {
            if (typeof candidate === "string" && candidate.trim()) {
                return candidate.trim();
            }
        }

        const detail = infoError ? `：${errorMessage(infoError)}` : "";
        throw new Error(`无法读取 Eagle 当前图库路径${detail}`);
    }

    function normalizeLibraryItems(result: unknown, apiName: string): EagleImage[] {
        if (Array.isArray(result)) return result as EagleImage[];
        if (
            result &&
            typeof result === "object" &&
            Array.isArray((result as { items?: unknown }).items)
        ) {
            return (result as { items: EagleImage[] }).items;
        }
        throw new Error(`${apiName} 未返回条目数组`);
    }

    async function listLibraryItems(
        hasRequiredFields: (items: EagleImage[]) => boolean,
    ): Promise<EagleImage[]> {
        const attempts: Array<{
            name: string;
            run: () => Promise<unknown>;
        }> = [];
        if (typeof eagle.item?.get === "function") {
            attempts.push({
                name: "eagle.item.get",
                run: () =>
                    eagle.item.get({
                        fields: [
                            "id",
                            "name",
                            "ext",
                            "filePath",
                            "thumbnailPath",
                            "modifiedAt",
                            "width",
                            "height",
                            "isDeleted",
                            "tags",
                            "annotation",
                        ],
                    }),
            });
        }
        if (typeof eagle.item?.list === "function") {
            attempts.push({
                name: "eagle.item.list",
                run: () => eagle.item.list({ limit: 999999 }),
            });
        }
        if (typeof eagle.item?.getAll === "function") {
            attempts.push({
                name: "eagle.item.getAll",
                run: () => eagle.item.getAll(),
            });
        }

        let emptyResult: EagleImage[] | undefined;
        const failures: string[] = [];
        for (const attempt of attempts) {
            try {
                const items = normalizeLibraryItems(await attempt.run(), attempt.name);
                if (items.length === 0) {
                    emptyResult = items;
                    continue;
                }
                if (hasRequiredFields(items)) return items;
                failures.push(`${attempt.name} 返回字段不完整`);
            } catch (error) {
                failures.push(`${attempt.name}: ${errorMessage(error)}`);
            }
        }

        if (emptyResult) return emptyResult;
        const detail = failures.length ? `：${failures.join("；")}` : "";
        throw new Error(`无法读取 Eagle 图库条目${detail}`);
    }

    async function getLibraryImages(): Promise<EagleImage[]> {
        const items = await listLibraryItems((entries) =>
            entries.some((item) => Boolean(item.filePath || item.thumbnailPath)),
        );
        return items.filter(
            (item) =>
                !item.isDeleted &&
                IMAGE_EXTENSIONS.has(itemExtension(item)) &&
                Boolean(item.filePath || item.thumbnailPath),
        );
    }

    function itemExtension(item: EagleImage) {
        const explicit = String(item.ext || "").trim().toLowerCase().replace(/^\./, "");
        if (explicit) return explicit;
        const source = item.filePath || item.thumbnailPath || item.name || "";
        return source.split(/[\\/]/).pop()?.match(/\.([^.]+)$/)?.[1].toLowerCase() || "";
    }

    async function getLibraryItemIds(): Promise<string[]> {
        const items = await listLibraryItems((entries) =>
            entries.some((item) => typeof item.id === "string" && item.id),
        );
        const ids = items
            .filter((item) => !item.isDeleted && typeof item.id === "string")
            .map((item) => item.id as string)
            .filter(Boolean);
        if (ids.length === 0 && items.length > 0) {
            throw new Error("无法完整读取图库条目 ID，已取消索引清理");
        }
        return [...new Set(ids)];
    }

    function applyLibraryCounts(images: EagleImage[]) {
        totalImages.value = images.length;
        tagTotalItems.value = images.length;
        const tags = new Set<string>();
        for (const image of images) {
            for (const tag of Array.isArray(image.tags) ? image.tags : []) {
                const normalized = String(tag).trim();
                if (normalized) tags.add(normalized);
            }
        }
        libraryTagCount.value = tags.size;
        libraryAnnotationCount.value = images.filter((image) => String(image.annotation || "").trim()).length;
        libraryCountsReady.value = true;
    }

    async function refreshLibraryCounts(retryEmpty = false) {
        const attemptCount = retryEmpty ? 4 : 1;
        for (let attempt = 0; attempt < attemptCount; attempt++) {
            try {
                const images = await getLibraryImages();
                if (images.length === 0 && attempt + 1 < attemptCount) {
                    await delay(250 * 2 ** attempt);
                    continue;
                }
                applyLibraryCounts(images);
                libraryReadError.value = "";
                return;
            } catch (error) {
                if (attempt + 1 < attemptCount) {
                    await delay(250 * 2 ** attempt);
                    continue;
                }
                libraryCountsReady.value = false;
                libraryReadError.value = errorMessage(error);
                console.error("读取 Eagle 图库统计失败", error);
                throw error;
            }
        }
    }

    function toEmbeddingInput(item: EagleImage): EmbeddingImageInput {
        return {
            id: item.id,
            path: item.filePath || item.thumbnailPath || "",
            name: item.name || "",
            annotation: item.annotation || "",
            modifiedAt: Number(item.modifiedAt) || 0,
        };
    }

    function toTagInput(item: EagleImage): EmbeddingTagInput {
        return {
            itemId: item.id,
            tags: Array.isArray(item.tags) ? item.tags : [],
        };
    }
    function toAnnotationInput(item: EagleImage): EmbeddingAnnotationInput { return { itemId: item.id, annotation: String(item.annotation || ""), updatedAt: Number(item.modifiedAt) || 0 }; }

    async function startAnnotationIndexing(targetItems?: EagleImage[], force = false) {
        if (isIndexing.value || isTagIndexing.value || isAnnotationIndexing.value || isSearching.value) return;
        const taskId = beginTask("embedding", "全局注释向量化"); if (!taskId) return; activeSemanticTaskId.value = taskId;
        isAnnotationIndexing.value = true; annotationIndexFailures.value = []; annotationIndexStatus.value = "正在加载模型";
        try {
            await ensureSession(); const images = targetItems || await getLibraryImages(); applyLibraryCounts(images);
            annotationTotalItems.value = images.length; annotationProcessedItems.value = 0;
            const size = Math.max(20, batchSize.value * 4);
            for (let offset = 0; offset < images.length; offset += size) {
                await waitForTaskControl(taskId);
                const batch = images.slice(offset, offset + size); annotationIndexStatus.value = `正在处理 ${offset + 1}-${Math.min(offset + batch.length, images.length)}`;
                const result = await getBackendClient().indexEmbeddingAnnotations(SESSION_ID, batch.map(toAnnotationInput), batchSize.value, force);
                annotationProcessedItems.value += batch.length; annotationIndexedCount.value = result.totalAnnotations;
                annotationIndexFailures.value.push(...result.failures.map((failure) => `${failure.itemId}：${failure.error}`));
                updateTask(taskId, { detail: annotationIndexStatus.value, completed: annotationProcessedItems.value, total: images.length });
            }
            if (!targetItems) { const pruned = await getBackendClient().pruneEmbeddingAnnotations(SESSION_ID, images.map((image) => image.id)); annotationIndexedCount.value = pruned.totalAnnotations; }
            annotationIndexStatus.value = annotationIndexFailures.value.length ? "完成，部分注释失败" : "注释索引完成"; completeTask(taskId, annotationIndexStatus.value);
        } catch (error) { if (isTaskCancelled(error)) { annotationIndexStatus.value = "已取消"; cancelTask(taskId); } else { annotationIndexStatus.value = "注释索引失败"; failTask(taskId, error); notification(errorMessage(error), "error"); } }
        finally { isAnnotationIndexing.value = false; activeSemanticTaskId.value = ""; }
    }

    async function startIndexing(targetItems?: EagleImage[], force = false) {
        if (isTagIndexing.value) return;
        if (isIndexing.value) {
            return;
        }
        if (backenAPI.is_processing) {
            notification("另一个任务正在运行", "warning");
            return;
        }
        const taskId = beginTask("embedding", "全局图片向量化");
        if (!taskId) return;
        activeSemanticTaskId.value = taskId;
        try {
            updateTask(taskId, { detail: "正在加载模型" });
            await ensureSession();
            const images = targetItems || await getLibraryImages();
            applyLibraryCounts(images);
            if (images.length === 0) {
                throw new Error("没有找到可索引的图片，已保留现有索引");
            }
            if (!targetItems) {
                const itemIds = await getLibraryItemIds();
                const pruneResult = await getBackendClient().pruneEmbedding(SESSION_ID, itemIds);
                indexedCount.value = pruneResult.totalIndexed;
            }
            totalImages.value = images.length;
            updateTask(taskId, {
                detail: "正在建立索引",
                total: images.length,
            });
            processedImages.value = 0;
            indexedThisRun.value = 0;
            skippedThisRun.value = 0;
            indexFailures.value = [];
            isIndexing.value = true;

            for (
                let offset = 0;
                offset < images.length && !disposed;
                offset += batchSize.value
            ) {
                await waitForTaskControl(taskId);
                if (disposed) break;
                const batch = images.slice(offset, offset + batchSize.value);
                indexStatus.value = `正在处理 ${offset + 1}-${Math.min(offset + batch.length, images.length)}`;
                const result = await getBackendClient().indexEmbeddingBatch(
                    SESSION_ID,
                    batch.map(toEmbeddingInput),
                    force,
                );
                processedImages.value += batch.length;
                updateTask(taskId, {
                    detail: indexStatus.value,
                    completed: processedImages.value,
                });
                indexedThisRun.value += result.indexedIds.length;
                skippedThisRun.value += result.skippedIds.length;
                indexFailures.value.push(...result.failures);
                for (const failure of result.failures) {
                    recordFailure({
                        taskId,
                        kind: "embedding",
                        itemId: failure.id,
                        name: failure.id,
                        path: failure.path,
                        error: failure.error,
                    });
                }
                indexedCount.value = result.totalIndexed;
            }
            if (!disposed) {
                indexStatus.value = indexFailures.value.length
                    ? "完成，部分图片失败"
                    : "索引完成";
                notification(
                    `新增 ${indexedThisRun.value}，跳过 ${skippedThisRun.value}`,
                    indexFailures.value.length ? "warning" : "success",
                );
                completeTask(taskId, indexStatus.value);
            }
        } catch (error) {
            if (isTaskCancelled(error)) { indexStatus.value = "已取消"; cancelTask(taskId); }
            else { indexStatus.value = "索引失败"; failTask(taskId, error); notification(errorMessage(error), "error"); }
        } finally {
            activeSemanticTaskId.value = "";
            isIndexing.value = false;
        }
    }

    async function startTagIndexing(targetItems?: EagleImage[], force = false) {
        if (isIndexing.value || isTagIndexing.value || isAnnotationIndexing.value || isSearching.value) return;
        if (backenAPI.is_processing) {
            notification("另一个任务正在运行", "warning");
            return;
        }
        const taskId = beginTask("embedding", "全局标签向量化");
        if (!taskId) return;
        activeSemanticTaskId.value = taskId;
        isTagIndexing.value = true;
        tagIndexFailures.value = [];
        tagIndexStatus.value = "正在加载模型";
        try {
            await ensureSession();
            const images = targetItems || await getLibraryImages();
            applyLibraryCounts(images);
            if (images.length === 0) throw new Error("没有找到可处理的图片");
            tagTotalItems.value = images.length;
            tagProcessedItems.value = 0;
            const tagBatchSize = Math.max(20, batchSize.value * 4);
            const forcedTags = new Set<string>();
            updateTask(taskId, {
                detail: "正在向量化标签",
                total: images.length,
            });
            for (let offset = 0; offset < images.length; offset += tagBatchSize) {
                await waitForTaskControl(taskId);
                const batch = images.slice(offset, offset + tagBatchSize);
                const forceTagIds = force
                    ? batch.flatMap((item) => item.tags || []).filter((tag) => {
                          const normalized = tag.trim();
                          if (!normalized || forcedTags.has(normalized)) return false;
                          forcedTags.add(normalized);
                          return true;
                      })
                    : [];
                tagIndexStatus.value = `正在处理 ${offset + 1}-${Math.min(offset + batch.length, images.length)}`;
                const result = await getBackendClient().indexEmbeddingTags(
                    SESSION_ID,
                    batch.map(toTagInput),
                    batchSize.value,
                    forceTagIds.length > 0,
                    forceTagIds,
                );
                tagProcessedItems.value += batch.length;
                tagIndexedCount.value = result.totalTags;
                tagLinkCount.value = result.totalLinks;
                tagIndexFailures.value.push(
                    ...result.failures.map(
                        (failure) => `${failure.tag}：${failure.error}`,
                    ),
                );
                updateTask(taskId, {
                    detail: tagIndexStatus.value,
                    completed: tagProcessedItems.value,
                });
            }
            if (!targetItems) { const pruned = await getBackendClient().pruneEmbeddingTags(SESSION_ID, images.map((image) => image.id)); tagIndexedCount.value = pruned.totalTags; tagLinkCount.value = pruned.totalLinks; }
            tagIndexStatus.value = tagIndexFailures.value.length
                ? "完成，部分标签失败"
                : "标签索引完成";
            completeTask(taskId, tagIndexStatus.value);
            notification(
                `已索引 ${tagIndexedCount.value} 个标签`,
                tagIndexFailures.value.length ? "warning" : "success",
            );
        } catch (error) {
            if (isTaskCancelled(error)) { tagIndexStatus.value = "已取消"; cancelTask(taskId); }
            else { tagIndexStatus.value = "标签索引失败"; failTask(taskId, error); notification(errorMessage(error), "error"); }
        } finally {
            isTagIndexing.value = false;
            activeSemanticTaskId.value = "";
        }
    }

    function filterNegativeHits(
        hits: EmbeddingSearchHit[],
        negativeHits: EmbeddingSearchHit[],
        penaltyWeight: number,
    ): EmbeddingSearchHit[] {
        if (negativeHits.length === 0) return hits;
        const negativeScores = new Map(
            negativeHits.map((hit) => [hit.itemId, hit.similarity]),
        );
        const normalizedWeight = normalizeNegativePromptWeight(penaltyWeight);
        return hits
            .map((hit) => {
                const negativeScore = negativeScores.get(hit.itemId) || 0;
                return {
                    ...hit,
                    similarity: Math.max(
                        0,
                        hit.similarity - negativeScore * normalizedWeight,
                    ),
                };
            })
            .sort((left, right) => right.similarity - left.similarity);
    }

    async function checkIndexHealth() {
        if (backenAPI.is_processing) {
            notification("另一个任务正在运行", "warning");
            return;
        }
        const taskId = beginTask("embedding", "索引健康检查");
        if (!taskId) return;
        isCheckingHealth.value = true;
        try {
            updateTask(taskId, { detail: "正在比对 Eagle 图片与索引" });
            await ensureSession();
            const images = await getLibraryImages();
            const itemIds = await getLibraryItemIds();
            applyLibraryCounts(images);
            healthResult.value = await getBackendClient().embeddingHealth(
                SESSION_ID,
                itemIds,
            );
            for (const item of healthResult.value.missingFiles) {
                recordFailure({
                    taskId,
                    kind: "embedding",
                    itemId: item.itemId,
                    name: item.itemId,
                    path: item.sourceUri,
                    error: "索引对应的图片文件不存在",
                });
            }
            completeTask(taskId, "检查完成");
        } catch (error) {
            failTask(taskId, error);
            notification(errorMessage(error), "error");
        } finally {
            isCheckingHealth.value = false;
        }
    }

    async function cleanStaleIndex() {
        if (!healthResult.value?.staleItems.length) return;
        if (backenAPI.is_processing) {
            notification("另一个任务正在运行", "warning");
            return;
        }
        const taskId = beginTask("embedding", "清理冗余索引");
        if (!taskId) return;
        try {
            const images = await getLibraryImages();
            const itemIds = await getLibraryItemIds();
            if (images.length === 0) {
                throw new Error("没有找到当前图片，已取消清理");
            }
            updateTask(taskId, { detail: "正在删除冗余向量" });
            const result = await getBackendClient().pruneEmbedding(
                SESSION_ID,
                itemIds,
            );
            indexedCount.value = result.totalIndexed;
            healthResult.value = await getBackendClient().embeddingHealth(
                SESSION_ID,
                itemIds,
            );
            completeTask(taskId, `已清理 ${result.removedCount} 条`);
            notification(`已清理 ${result.removedCount} 条冗余索引`, "success");
        } catch (error) {
            failTask(taskId, error);
            notification(errorMessage(error), "error");
        }
    }

    async function runSearch() {
        if (isIndexing.value || isTagIndexing.value || isAnnotationIndexing.value || isSearching.value) return;
        if (backenAPI.is_processing) {
            notification("另一个任务正在运行", "warning");
            return;
        }
        isSearching.value = true;
        const taskId = beginTask("search", "语义搜索");
        if (!taskId) {
            isSearching.value = false;
            return;
        }
        const generation = resetSearchResults();
        try {
            updateTask(taskId, { detail: "正在生成查询向量" });
            await ensureSession();
            const backend = getBackendClient();
            const useTagFusion =
                searchMode.value === "text" &&
                includeTags.value &&
                canIncludeTags.value;
            if (searchMode.value === "text" && !includeImages.value && !useTagFusion && !(includeAnnotations.value && canIncludeAnnotations.value)) {
                throw new Error("请至少选择图片、注释或标签向量中的一种");
            }
            const searchableCount = Math.max(indexedCount.value, tagLinkCount.value, annotationIndexedCount.value);
            const resultCount = Math.max(
                1,
                Math.min(searchableCount, MAX_SEARCH_RESULTS),
            );
            let hits: EmbeddingSearchHit[];
            if (searchMode.value === "text") {
                if (!queryText.value.trim()) throw new Error("请输入搜索文字");
                hits = (
                    await backend.searchEmbeddingText(
                        SESSION_ID,
                        queryText.value,
                        resultCount,
                        includeImages.value && canIncludeImages.value,
                        useTagFusion,
                        includeAnnotations.value && canIncludeAnnotations.value,
                    )
                ).hits;
            } else {
                const selected: EagleImage[] = await eagle.item.getSelected();
                if (selected.length !== 1) {
                    throw new Error("请在 Eagle 中选择一张图片");
                }
                const item = selected[0];
                if (
                    !IMAGE_EXTENSIONS.has(
                        String(item.ext || "").toLowerCase(),
                    )
                ) {
                    throw new Error("当前选中项目不是支持的图片格式");
                }
                const imagePath = item.filePath || item.thumbnailPath;
                if (!imagePath) throw new Error("选中项目没有可读取的图片");
                hits = (
                    await backend.searchEmbeddingImage(
                        SESSION_ID,
                        imagePath,
                        item.id,
                        Number(item.modifiedAt) || 0,
                        resultCount,
                    )
                ).hits;
            }
            if (negativeQueryText.value.trim()) {
                const negativeHits = (
                    await backend.searchEmbeddingText(
                        SESSION_ID,
                        negativeQueryText.value,
                        resultCount,
                        includeImages.value && canIncludeImages.value,
                        useTagFusion,
                        includeAnnotations.value && canIncludeAnnotations.value,
                    )
                ).hits;
                hits = filterNegativeHits(
                    hits,
                    negativeHits,
                    negativePromptWeight.value,
                );
            }
            if (generation !== searchGeneration) return;
            allSearchHits.value = [...hits].sort(
                (left, right) => right.similarity - left.similarity,
            );
            await loadMoreSearchResults(generation);
            if (generation !== searchGeneration) return;
            completeTask(taskId, `找到 ${allSearchHits.value.length} 个结果`);
        } catch (error) {
            failTask(taskId, error);
            notification(errorMessage(error), "error");
        } finally {
            isSearching.value = false;
        }
    }

    function resetSearchResults() {
        searchGeneration += 1;
        allSearchHits.value = [];
        searchResults.value = [];
        nextSearchHitOffset.value = 0;
        isLoadingMoreResults.value = false;
        expandedResult.value = null;
        if (previewClickTimer) clearTimeout(previewClickTimer);
        previewClickTimer = undefined;
        return searchGeneration;
    }

    async function loadMoreSearchResults(generation = searchGeneration) {
        if (
            generation !== searchGeneration ||
            isLoadingMoreResults.value ||
            !hasMoreSearchResults.value
        ) {
            return;
        }
        isLoadingMoreResults.value = true;
        const start = nextSearchHitOffset.value;
        const end = Math.min(
            start + SEARCH_RESULT_PAGE_SIZE,
            allSearchHits.value.length,
        );
        try {
            const hydrated = await hydrateSearchResults(
                allSearchHits.value.slice(start, end),
            );
            if (generation !== searchGeneration) return;
            searchResults.value.push(...hydrated);
            nextSearchHitOffset.value = end;
            await nextTick();
            refreshMasonryLayout();
        } catch (error) {
            if (generation === searchGeneration) {
                notification(errorMessage(error), "error");
            }
        } finally {
            if (generation === searchGeneration) {
                isLoadingMoreResults.value = false;
            }
        }
    }

    async function hydrateSearchResults(
        hits: EmbeddingSearchHit[],
    ): Promise<SearchDisplayItem[]> {
        if (hits.length === 0) return [];
        const items: EagleImage[] = await eagle.item.getByIds(
            hits.map((hit) => hit.itemId),
        );
        const byId = new Map(items.map((item) => [item.id, item]));
        return hits.flatMap((hit) => {
            const item = byId.get(hit.itemId);
            if (!item) return [];
            const thumbnailPath =
                item.thumbnailPath || item.filePath || hit.sourceUri;
            const previewPath = item.filePath || item.thumbnailPath || hit.sourceUri;
            if (!thumbnailPath || !previewPath) return [];
            const ratio =
                Number(item.width) > 0 && Number(item.height) > 0
                    ? `${item.width} / ${item.height}`
                    : "1 / 1";
            return [
                {
                    ...hit,
                    displayName: item.name || hit.name || hit.itemId,
                    thumbnailUrl: localImageUrl(thumbnailPath),
                    previewUrl: localImageUrl(previewPath),
                    aspectRatio: ratio,
                },
            ];
        });
    }

    function previewResult(result: SearchDisplayItem) {
        if (previewClickTimer) clearTimeout(previewClickTimer);
        previewClickTimer = setTimeout(() => {
            expandedResult.value = result;
            previewClickTimer = undefined;
        }, 220);
    }

    function closePreview() {
        expandedResult.value = null;
    }

    async function openResult(itemId: string) {
        if (previewClickTimer) clearTimeout(previewClickTimer);
        previewClickTimer = undefined;
        closePreview();
        await eagle.item.open(itemId);
    }

    function onResultImageLoad(event: Event) {
        const image = event.currentTarget as HTMLElement | null;
        const card = image?.closest<HTMLElement>(".result-card");
        if (card) layoutMasonryCard(card);
    }

    function refreshMasonryLayout() {
        requestAnimationFrame(() => {
            masonryGrid.value
                ?.querySelectorAll<HTMLElement>(".result-card")
                .forEach(layoutMasonryCard);
        });
    }

    function layoutMasonryCard(card: HTMLElement) {
        const grid = masonryGrid.value;
        if (!grid) return;
        const styles = getComputedStyle(grid);
        const rowHeight = Number.parseFloat(styles.gridAutoRows);
        const rowGap = Number.parseFloat(styles.rowGap);
        if (!(rowHeight > 0) || !Number.isFinite(rowGap)) return;
        card.style.gridRowEnd = "auto";
        const span = Math.ceil(
            (card.getBoundingClientRect().height + rowGap) /
                (rowHeight + rowGap),
        );
        card.style.gridRowEnd = `span ${Math.max(1, span)}`;
    }

    function localImageUrl(filePath: string) {
        if (/^(?:data:|https?:|file:)/i.test(filePath)) return filePath;
        return pathToFileURL(filePath).href;
    }

    function similarityText(similarity: number) {
        const bounded = Math.max(-1, Math.min(1, similarity));
        return bounded.toFixed(3);
    }

    function errorMessage(error: unknown) {
        return error instanceof Error ? error.message : String(error);
    }

    function confirmForceIndexing(target: string, action: () => Promise<void>) {
        dialog.warning({
            title: "确认强制全部向量化",
            content: `将忽略已有${target}向量，重新计算当前图库内的全部${target}。此操作可能耗时较长。`,
            positiveText: "强制全部向量化",
            negativeText: "取消",
            onPositiveClick: action,
        });
    }

    function delay(milliseconds: number) {
        return new Promise((resolve) => setTimeout(resolve, milliseconds));
    }
    async function runImageIndexing(items: EagleImage[]) {
        await startIndexing(items);
        if (indexStatus.value === "索引失败") throw new Error(indexStatus.value);
        if (indexFailures.value.length > 0) {
            throw new Error(`${indexFailures.value.length} 张图片向量化失败`);
        }
        if (processedImages.value < items.length) {
            throw new Error("图片向量化任务未完整执行");
        }
    }

    async function runTagIndexing(items: EagleImage[]) {
        await startTagIndexing(items);
        if (tagIndexStatus.value === "标签索引失败") throw new Error(tagIndexStatus.value);
        if (tagIndexFailures.value.length > 0) {
            throw new Error(`${tagIndexFailures.value.length} 个标签向量化失败`);
        }
        if (tagProcessedItems.value < items.length) {
            throw new Error("标签向量化任务未完整执行");
        }
    }

    async function runAnnotationIndexing(items: EagleImage[]) {
        await startAnnotationIndexing(items);
        if (annotationIndexStatus.value === "注释索引失败") {
            throw new Error(annotationIndexStatus.value);
        }
        if (annotationIndexFailures.value.length > 0) {
            throw new Error(`${annotationIndexFailures.value.length} 条注释向量化失败`);
        }
        if (annotationProcessedItems.value < items.length) {
            throw new Error("注释向量化任务未完整执行");
        }
    }

    defineExpose({
        ready: isReady,
        runImageIndexing,
        runTagIndexing,
        runAnnotationIndexing,
    });
</script>

<template>
    <main class="semantic-page">
        <header class="model-toolbar">
            <div class="model-field">
                <span class="field-label">向量模型</span>
                <FormHelp :content="t('semantic_search.model_desc')" />
                <n-select
                    v-model:value="selectedModel"
                    :options="modelOptions"
                    :loading="isLoadingModels"
                    :disabled="isIndexing || isTagIndexing || isSearching"
                    placeholder="未检测到可用模型"
                    class="model-select"
                />
            </div>
            <n-button
                type="primary"
                secondary
                :disabled="isIndexing || isTagIndexing || isSearching"
                @click="showDownload = true"
            >
                <template #icon>
                    <n-icon><CloudDownloadOutline /></n-icon>
                </template>
                下载模型
            </n-button>
            <n-tag
                v-if="loadedModelKey === selectedModel"
                size="small"
                :bordered="false"
            >
                已索引 {{ indexedCount }} 张
            </n-tag>
            <n-tag
                v-if="loadedModelKey === selectedModel && tagIndexedCount > 0"
                size="small"
                :bordered="false"
                type="success"
            >
                已索引 {{ tagIndexedCount }} 个标签
            </n-tag>
        </header>

        <n-alert
            v-if="libraryReadError"
            title="图库读取失败"
            type="error"
            class="failure-alert"
        >
            {{ libraryReadError }}
        </n-alert>

        <TaskControlButtons v-if="activeSemanticTaskId" :task-id="activeSemanticTaskId" class="semantic-task-controls" />
        <n-tabs v-model:value="activeTab" type="line" animated class="feature-tabs" :on-before-leave="() => !activeSemanticTaskId">
            <n-tab-pane name="index" tab="全局图片向量化">
                <section class="index-panel">
                    <div class="metrics-row">
                        <div class="metric">
                            <span>总图片</span>
                            <strong>{{ totalImages }}</strong>
                        </div>
                        <div class="metric metric--green">
                            <span>已向量化</span>
                            <strong>{{ indexedCount }}</strong>
                        </div>
                        <div class="metric">
                            <span>待向量化</span>
                            <strong>{{ imagePendingCount }}</strong>
                        </div>
                        <div class="metric metric--red">
                            <span>失败</span>
                            <strong>{{ indexFailures.length }}</strong>
                        </div>
                    </div>

                    <div class="progress-block">
                        <div class="progress-heading">
                            <span>{{ indexStatus }}</span>
                            <span>{{ processedImages }}/{{ totalImages }}</span>
                        </div>
                        <n-progress
                            type="line"
                            :percentage="indexPercentage"
                            :processing="isIndexing"
                            indicator-placement="inside"
                        />
                    </div>

                    <div class="index-actions">
                        <div class="batch-field">
                            <span class="field-label">{{ batchFieldLabel }}</span>
                            <FormHelp
                                :content="
                                    t(
                                        isRemoteModel
                                            ? 'semantic_search.concurrency_desc'
                                            : 'semantic_search.batch_size_desc',
                                    )
                                "
                            />
                            <n-input-number
                                v-model:value="batchSize"
                                :min="1"
                                :max="batchFieldMax"
                                :disabled="isIndexing"
                            />
                        </div>
                        <n-space>
                            <n-button
                                :loading="isCheckingHealth"
                                :disabled="!selectedModel || isIndexing"
                                @click="checkIndexHealth"
                            >
                                <template #icon>
                                    <n-icon><PulseOutline /></n-icon>
                                </template>
                                健康检查
                            </n-button>
                            <n-button
                                v-if="!isIndexing"
                                type="warning"
                                secondary
                                :disabled="!selectedModel || isTagIndexing || isAnnotationIndexing || isSearching"
                                @click="confirmForceIndexing('图片', () => startIndexing(undefined, true))"
                            >
                                <template #icon>
                                    <n-icon><ReloadOutline /></n-icon>
                                </template>
                                强制全部向量化
                            </n-button>
                            <n-button
                                v-if="!isIndexing"
                                type="primary"
                                :disabled="!selectedModel"
                                @click="() => startIndexing()"
                            >
                                <template #icon>
                                    <n-icon><PlayOutline /></n-icon>
                                </template>
                                向量化未处理图片
                            </n-button>
                        </n-space>
                    </div>

                    <section v-if="healthResult" class="health-panel">
                        <div class="health-metrics">
                            <div><span>库内图片</span><strong>{{ healthResult.libraryCount }}</strong></div>
                            <div><span>已索引</span><strong>{{ healthResult.indexedCount }}</strong></div>
                            <div><span>待索引</span><strong>{{ healthResult.missingItemIds.length }}</strong></div>
                            <div><span>冗余</span><strong>{{ healthResult.staleItems.length }}</strong></div>
                            <div><span>文件丢失</span><strong>{{ healthResult.missingFiles.length }}</strong></div>
                        </div>
                        <n-button
                            v-if="healthResult.staleItems.length"
                            type="warning"
                            secondary
                            @click="cleanStaleIndex"
                        >
                            <template #icon><n-icon><TrashOutline /></n-icon></template>
                            清理冗余索引
                        </n-button>
                    </section>

                    <n-alert
                        v-if="indexFailures.length"
                        title="未能处理的图片"
                        type="warning"
                        class="failure-alert"
                    >
                        <div
                            v-for="failure in indexFailures.slice(0, 6)"
                            :key="`${failure.id}-${failure.path}`"
                            class="failure-line"
                        >
                            {{ failure.path }}：{{ failure.error }}
                        </div>
                        <div v-if="indexFailures.length > 6">
                            另有 {{ indexFailures.length - 6 }} 张
                        </div>
                    </n-alert>
                </section>
            </n-tab-pane>

            <n-tab-pane name="tag-index" tab="标签向量化">
                <section class="index-panel">
                    <div class="metrics-row">
                        <div class="metric">
                            <span>总标签</span>
                            <strong>{{ tagTotalCount }}</strong>
                        </div>
                        <div class="metric metric--green">
                            <span>已向量化</span>
                            <strong>{{ tagIndexedCount }}</strong>
                        </div>
                        <div class="metric">
                            <span>待向量化</span>
                            <strong>{{ tagPendingCount }}</strong>
                        </div>
                        <div class="metric metric--red">
                            <span>失败</span>
                            <strong>{{ tagIndexFailures.length }}</strong>
                        </div>
                    </div>
                    <div class="progress-block">
                        <div class="progress-heading">
                            <span>{{ tagIndexStatus }}</span>
                            <span>{{ tagProcessedItems }}/{{ tagTotalItems }}</span>
                        </div>
                        <n-progress
                            type="line"
                            :percentage="tagIndexPercentage"
                            :processing="isTagIndexing"
                            indicator-placement="inside"
                        />
                    </div>
                    <div class="index-actions">
                        <div class="batch-field">
                            <span class="field-label">{{ batchFieldLabel }}</span>
                            <FormHelp
                                :content="
                                    t(
                                        isRemoteModel
                                            ? 'semantic_search.concurrency_desc'
                                            : 'semantic_search.batch_size_desc',
                                    )
                                "
                            />
                            <n-input-number
                                v-model:value="batchSize"
                                :min="1"
                                :max="batchFieldMax"
                                :disabled="isTagIndexing"
                            />
                        </div>
                        <span class="field-label">已建立 {{ tagLinkCount }} 条图片-标签关系</span>
                        <n-space>
                            <n-button
                                type="warning"
                                secondary
                                :disabled="!selectedModel || isIndexing || isTagIndexing || isAnnotationIndexing || isSearching"
                                @click="confirmForceIndexing('标签文本', () => startTagIndexing(undefined, true))"
                            >
                                <template #icon><n-icon><ReloadOutline /></n-icon></template>
                                强制全部向量化
                            </n-button>
                            <n-button
                                type="primary"
                                :loading="isTagIndexing"
                                :disabled="!selectedModel || isIndexing || isAnnotationIndexing || isSearching"
                                @click="() => startTagIndexing()"
                            >
                                <template #icon>
                                    <n-icon><PricetagsOutline /></n-icon>
                                </template>
                                {{ isTagIndexing ? "正在向量化" : "向量化未处理标签" }}
                            </n-button>
                        </n-space>
                    </div>
                    <n-alert
                        v-if="tagIndexFailures.length"
                        title="未能处理的标签"
                        type="warning"
                        class="failure-alert"
                    >
                        <div v-for="failure in tagIndexFailures.slice(0, 8)" :key="failure">
                            {{ failure }}
                        </div>
                    </n-alert>
                </section>
            </n-tab-pane>

            <n-tab-pane name="annotation-index" tab="注释向量化">
                <section class="index-panel">
                    <div class="metrics-row">
                        <div class="metric"><span>总注释</span><strong>{{ libraryAnnotationCount }}</strong></div>
                        <div class="metric metric--green"><span>已向量化</span><strong>{{ annotationIndexedCount }}</strong></div>
                        <div class="metric"><span>待向量化</span><strong>{{ annotationPendingCount }}</strong></div>
                        <div class="metric metric--red"><span>失败</span><strong>{{ annotationIndexFailures.length }}</strong></div>
                    </div>
                    <div class="progress-block"><div class="progress-heading"><span>{{ annotationIndexStatus }}</span><span>{{ annotationProcessedItems }}/{{ annotationTotalItems }}</span></div><n-progress type="line" :percentage="annotationIndexPercentage" :processing="isAnnotationIndexing" indicator-placement="inside" /></div>
                    <div class="index-actions">
                        <span class="field-label">每张图片最多建立一条注释向量</span>
                        <n-space>
                            <n-button
                                type="warning"
                                secondary
                                :disabled="!selectedModel || isIndexing || isTagIndexing || isAnnotationIndexing || isSearching"
                                @click="confirmForceIndexing('注释', () => startAnnotationIndexing(undefined, true))"
                            >
                                <template #icon><n-icon><ReloadOutline /></n-icon></template>
                                强制全部向量化
                            </n-button>
                            <n-button
                                type="primary"
                                :loading="isAnnotationIndexing"
                                :disabled="!selectedModel || isIndexing || isTagIndexing || isSearching"
                                @click="() => startAnnotationIndexing()"
                            >
                                <template #icon><n-icon><InformationCircleOutline /></n-icon></template>
                                {{ isAnnotationIndexing ? "正在向量化" : "向量化未处理注释" }}
                            </n-button>
                        </n-space>
                    </div>
                    <n-alert v-if="annotationIndexFailures.length" title="未能处理的注释" type="warning" class="failure-alert"><div v-for="failure in annotationIndexFailures.slice(0, 8)" :key="failure">{{ failure }}</div></n-alert>
                </section>
            </n-tab-pane>

            <n-tab-pane name="search" tab="相似图片搜索">
                <section class="search-panel">
                    <div class="search-toolbar">
                        <FormHelp
                            :content="t('semantic_search.search_mode_desc')"
                        />
                        <n-radio-group
                            v-model:value="searchMode"
                            :disabled="isSearching || isIndexing || isTagIndexing"
                        >
                            <n-radio-button value="text">文字</n-radio-button>
                            <n-radio-button value="image">当前图片</n-radio-button>
                        </n-radio-group>
                        <div class="vector-controls-grid">
                            <template v-if="searchMode === 'text'">
                                <div class="vector-control-block">
                                    <n-checkbox v-model:checked="includeImages" :disabled="isSearching || isIndexing || !canIncludeImages">
                                        <span class="field-label">{{ t("semantic_search.vector_image") }}</span>
                                    </n-checkbox>
                                </div>
                                <div class="vector-control-block">
                                    <n-checkbox v-model:checked="includeAnnotations" :disabled="isSearching || isIndexing || isAnnotationIndexing || !canIncludeAnnotations">
                                        <span class="field-label">{{ t("semantic_search.vector_annotation") }}</span>
                                    </n-checkbox>
                                </div>
                                <div class="vector-control-block">
                                    <n-checkbox v-model:checked="includeTags" :disabled="isSearching || isIndexing || isTagIndexing || !canIncludeTags">
                                        <span class="field-label">{{ t("semantic_search.vector_tag") }}</span>
                                    </n-checkbox>
                                </div>
                            </template>
                            <div class="vector-control-block vector-control-block--weight">
                                <span class="field-label">{{ t("semantic_search.negative_prompt_weight") }}</span>
                                <div class="negative-weight-control">
                                    <n-slider
                                        v-model:value="negativePromptWeight"
                                        :min="0"
                                        :max="1"
                                        :step="0.05"
                                        :disabled="isSearching || isIndexing || isTagIndexing"
                                    />
                                    <span class="negative-weight-value">{{ negativePromptWeight.toFixed(2) }}</span>
                                </div>
                            </div>
                        </div>
                        <span v-if="searchMode === 'text' && !canIncludeTags" class="field-label vector-hint">
                            完成标签向量化后可开启
                        </span>
                        <FormHelp
                            v-if="searchMode === 'text'"
                            :content="t('semantic_search.query_desc')"
                        />
                        <n-input
                            v-if="searchMode === 'text'"
                            v-model:value="queryText"
                            clearable
                            placeholder="输入要搜索的画面或概念"
                            :disabled="isSearching || isIndexing || isTagIndexing"
                            @keyup.enter="runSearch"
                        />
                        <n-input
                            v-model:value="negativeQueryText"
                            clearable
                            placeholder="负面搜索：排除的内容"
                            :disabled="isSearching || isIndexing || isTagIndexing"
                            @keyup.enter="runSearch"
                        />
                        <div v-if="searchMode === 'image'" class="selected-image-mode">
                            <n-icon :size="20"><ImageOutline /></n-icon>
                            <span>当前选中图片</span>
                        </div>
                        <n-button
                            type="primary"
                            :loading="isSearching"
                            :disabled="!selectedModel || isIndexing || isTagIndexing"
                            @click="runSearch"
                        >
                            <template #icon>
                                <n-icon><SearchOutline /></n-icon>
                            </template>
                            搜索
                        </n-button>
                    </div>

                    <div class="results-heading">
                        <div class="results-title">
                            <span>搜索结果</span>
                            <n-tooltip trigger="hover">
                                <template #trigger>
                                    <n-icon
                                        class="results-help-icon"
                                        :size="16"
                                    >
                                        <InformationCircleOutline />
                                    </n-icon>
                                </template>
                                {{ t("semantic_search.result_interaction_hint") }}
                            </n-tooltip>
                        </div>
                        <span>
                            {{ searchResults.length }} / {{ allSearchHits.length }}
                            <template v-if="isSearchResultCapped">
                                （最多 4096）
                            </template>
                        </span>
                    </div>

                    <div v-if="isSearching" class="loading-state">
                        <n-spin size="large" />
                        <span>正在计算相似度</span>
                    </div>
                    <n-empty
                        v-else-if="allSearchHits.length === 0"
                        description="暂无结果"
                        class="empty-state"
                    />
                    <template v-else>
                        <div ref="masonryGrid" class="masonry-grid">
                            <article
                                v-for="result in searchResults"
                                :key="result.itemId"
                                class="result-card"
                                :title="result.displayName"
                                @click="previewResult(result)"
                                @dblclick.stop="openResult(result.itemId)"
                            >
                                <div class="similarity-badge">
                                    相似度 {{ similarityText(result.similarity) }}
                                </div>
                                <img
                                    :src="result.thumbnailUrl"
                                    :alt="result.displayName"
                                    :style="{ aspectRatio: result.aspectRatio }"
                                    loading="lazy"
                                    draggable="false"
                                    @load="onResultImageLoad"
                                />
                                <div class="result-name">
                                    {{ result.displayName }}
                                </div>
                            </article>
                        </div>
                        <div
                            v-if="hasMoreSearchResults"
                            ref="searchLoadSentinel"
                            class="result-load-sentinel"
                        >
                            <n-spin v-if="isLoadingMoreResults" size="small" />
                        </div>
                    </template>
                </section>
            </n-tab-pane>
        </n-tabs>
    </main>

    <Teleport to="body">
        <div
            v-if="expandedResult"
            class="result-preview"
            @click.self="closePreview"
        >
            <div class="result-preview-content" @click.stop>
                <img
                    :src="expandedResult.previewUrl"
                    :alt="expandedResult.displayName"
                    draggable="false"
                    @dblclick.stop="openResult(expandedResult.itemId)"
                />
                <div class="result-preview-meta">
                    <span>{{ expandedResult.displayName }}</span>
                    <strong>
                        相似度 {{ similarityText(expandedResult.similarity) }}
                    </strong>
                </div>
                <div class="result-preview-hint">
                    <n-icon :size="16"><OpenOutline /></n-icon>
                    <span>{{ t("semantic_search.preview_open_hint") }}</span>
                </div>
            </div>
        </div>
    </Teleport>

    <downloadModal
        v-model:showModal="showDownload"
        model-type="embedding"
        :reload-on-complete="false"
        :initial-selection="
            selectedModelInfo?.provider === 'local' ? selectedModel : ''
        "
        @completed="refreshModels"
    />
</template>

<style scoped>
    .semantic-page {
        height: 100%;
        min-width: 0;
        overflow: auto;
        padding: 22px 28px 40px;
        box-sizing: border-box;
        background: #101214;
    }

    .model-toolbar,
    .search-toolbar,
    .index-actions {
        display: flex;
        align-items: center;
        gap: 12px;
        min-width: 0;
    }

    .model-toolbar {
        min-height: 44px;
        padding-bottom: 14px;
        border-bottom: 1px solid #2b2f33;
    }

    .model-field,
    .batch-field {
        display: flex;
        align-items: center;
        gap: 8px;
        min-width: 0;
    }

    .model-field {
        flex: 1;
        max-width: 560px;
    }

    .field-label {
        flex: 0 0 auto;
        color: #a8adb3;
        font-size: 13px;
    }

    .model-select {
        flex: 1;
        min-width: 180px;
    }

    .feature-tabs {
        margin-top: 6px;
    }

    .feature-tabs :deep(.n-tabs-pane-wrapper) {
        overflow: visible;
    }

    .index-panel,
    .search-panel {
        padding-top: 18px;
    }

    .metrics-row {
        display: grid;
        grid-template-columns: repeat(4, minmax(110px, 1fr));
        border-top: 1px solid #2b2f33;
        border-bottom: 1px solid #2b2f33;
    }

    .metric {
        min-width: 0;
        padding: 17px 18px;
        border-right: 1px solid #2b2f33;
    }

    .metric:last-child {
        border-right: 0;
    }

    .metric span {
        display: block;
        color: #8f969d;
        font-size: 12px;
    }

    .metric strong {
        display: block;
        margin-top: 4px;
        color: #f2f4f5;
        font-size: 23px;
        line-height: 1.2;
    }

    .metric--green strong {
        color: #63c28d;
    }

    .metric--red strong {
        color: #e27878;
    }

    .progress-block {
        margin-top: 26px;
    }

    .progress-heading,
    .results-heading {
        display: flex;
        justify-content: space-between;
        gap: 16px;
        margin-bottom: 9px;
        color: #b5bbc1;
        font-size: 13px;
    }

    .index-actions {
        justify-content: space-between;
        margin-top: 22px;
    }

    .batch-field :deep(.n-input-number) {
        width: 110px;
    }

    .failure-alert {
        margin-top: 22px;
    }

    .health-panel {
        margin-top: 18px;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
        padding: 12px;
        border-top: 1px solid rgba(255, 255, 255, 0.1);
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    }

    .health-metrics {
        display: flex;
        flex-wrap: wrap;
        gap: 18px;
    }

    .health-metrics div {
        display: grid;
        gap: 2px;
    }

    .health-metrics span {
        color: #aeb4bc;
        font-size: 12px;
    }

    .failure-line {
        overflow-wrap: anywhere;
        margin-bottom: 4px;
    }

    .search-toolbar {
        position: sticky;
        top: -22px;
        z-index: 4;
        flex-wrap: wrap;
        padding: 12px 0;
        background: #101214;
        border-bottom: 1px solid #2b2f33;
    }

    .search-toolbar :deep(.n-input) {
        flex: 1;
        min-width: 180px;
    }

    .vector-controls-grid {
        display: flex;
        align-items: center;
        flex-wrap: wrap;
        gap: 12px;
        flex: 0 0 100%;
        width: 100%;
        min-width: 0;
    }

    .vector-control-block {
        display: flex;
        align-items: center;
        gap: 8px;
        min-width: 0;
        padding: 0;
    }

    .vector-control-block--weight {
        min-width: 0;
    }

    .negative-weight-control {
        display: flex;
        align-items: center;
        gap: 10px;
        min-width: 0;
    }

    .negative-weight-control :deep(.n-slider) {
        flex: 0 0 120px;
        width: 120px;
    }

    .negative-weight-value {
        width: 38px;
        color: #c8cdd2;
        font-variant-numeric: tabular-nums;
        font-size: 12px;
        text-align: right;
    }

    .vector-hint {
        display: inline-block;
        margin-top: 6px;
    }

    .selected-image-mode {
        display: flex;
        align-items: center;
        gap: 8px;
        flex: 1;
        min-width: 180px;
        height: 34px;
        padding: 0 10px;
        box-sizing: border-box;
        color: #b8bec4;
        border: 1px solid #343a40;
        border-radius: 4px;
    }

    .results-heading {
        margin-top: 18px;
        padding-bottom: 8px;
        border-bottom: 1px solid #252a2e;
    }

    .results-title,
    .result-preview-hint {
        display: flex;
        align-items: center;
        gap: 6px;
    }

    .results-help-icon {
        flex: none;
        color: #858d94;
        cursor: help;
    }

    .results-help-icon:hover {
        color: #b8c0c7;
    }

    .loading-state,
    .empty-state {
        min-height: 280px;
    }

    .loading-state {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 14px;
        color: #92999f;
    }

    .masonry-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
        grid-auto-flow: row;
        grid-auto-rows: 8px;
        gap: 12px;
        align-items: start;
    }

    .result-card {
        position: relative;
        display: block;
        width: 100%;
        overflow: hidden;
        box-sizing: border-box;
        border: 1px solid #2d3237;
        border-radius: 6px;
        background: #191c1f;
        cursor: pointer;
    }

    .result-card:hover {
        border-color: #5f9f7b;
        background: #1e2225;
    }

    .result-card img {
        display: block;
        width: 100%;
        min-height: 100px;
        object-fit: cover;
        background: #24282c;
    }

    .similarity-badge {
        position: absolute;
        top: 7px;
        left: 7px;
        z-index: 2;
        padding: 3px 6px;
        color: #f4fff8;
        font-size: 12px;
        font-weight: 600;
        line-height: 1.3;
        border-radius: 4px;
        background: rgba(22, 92, 58, 0.9);
        box-shadow: 0 1px 4px rgba(0, 0, 0, 0.35);
    }

    .result-name {
        overflow: hidden;
        padding: 8px 9px;
        color: #d8dcdf;
        font-size: 12px;
        line-height: 1.35;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .result-load-sentinel {
        display: flex;
        align-items: center;
        justify-content: center;
        min-height: 72px;
    }

    .result-preview {
        position: fixed;
        inset: 0;
        z-index: 2000;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 32px;
        box-sizing: border-box;
        background: rgba(4, 6, 7, 0.88);
    }

    .result-preview-content {
        display: flex;
        flex-direction: column;
        align-items: stretch;
        gap: 10px;
        max-width: min(92vw, 1400px);
        max-height: 92vh;
    }

    .result-preview-content img {
        display: block;
        max-width: 100%;
        max-height: calc(92vh - 48px);
        object-fit: contain;
        background: #0d0f10;
        box-shadow: 0 10px 36px rgba(0, 0, 0, 0.5);
    }

    .result-preview-meta {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 20px;
        min-width: 0;
        color: #e4e8eb;
        font-size: 13px;
    }

    .result-preview-meta span {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .result-preview-meta strong {
        flex: none;
        color: #7ac99c;
    }

    .result-preview-hint {
        justify-content: center;
        color: #aeb5bb;
        font-size: 12px;
    }

    @media (max-width: 820px) {
        .semantic-page {
            padding-right: 18px;
            padding-left: 18px;
        }

        .model-toolbar,
        .search-toolbar {
            flex-wrap: wrap;
        }

        .model-field,
        .search-toolbar :deep(.n-input),
        .selected-image-mode {
            flex-basis: calc(100% - 150px);
        }

        .metrics-row {
            grid-template-columns: repeat(2, minmax(100px, 1fr));
        }

        .metric:nth-child(2) {
            border-right: 0;
        }

        .metric:nth-child(-n + 2) {
            border-bottom: 1px solid #2b2f33;
        }

        .masonry-grid {
            grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
        }
    }

    @media (max-width: 560px) {
        .model-field,
        .search-toolbar :deep(.n-input),
        .selected-image-mode {
            flex-basis: 100%;
        }

        .index-actions {
            align-items: flex-end;
        }

        .masonry-grid {
            grid-template-columns: repeat(2, minmax(0, 1fr));
        }

        .result-preview {
            padding: 16px;
        }
    }
</style>
