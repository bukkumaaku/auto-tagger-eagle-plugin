<script setup lang="ts">
    import {
        NButton,
        NForm,
        NFormItem,
        NInput,
        NInputNumber,
        NIcon,
        NProgress,
        NRadio,
        NRadioGroup,
        NSelect,
        NSpace,
        NTabPane,
        NTabs,
        NTag,
    } from "naive-ui";
    import { CloudDownloadOutline } from "@vicons/ionicons5";
    import {
        computed,
        h,
        nextTick,
        onBeforeUnmount,
        onMounted,
        ref,
        watch,
        type Ref,
    } from "vue";
    import {
        backenAPI,
        config,
        dialog,
        notification,
        t,
    } from "../api/backen";
    import { llmModelInfo } from "../api/modelInfo";
    import {
        DEFAULT_LLM_ANNOTATION_PROMPT,
        DEFAULT_LLM_TAG_PROMPT,
    } from "../constants/llm";
    import {
        getBackendClient,
        type BackendClient,
    } from "../services/backendClient";
    import {
        beginTask,
        completeTask,
        failTask,
        recordFailure,
        updateTask,
    } from "../services/taskCenter";
import downloadModal from "./downloadModal.vue";
import FormHelp from "./formHelp.vue";
import {
    createTaggerBackupInDirectory,
    listTaggerBackupsInDirectory,
    restoreTaggerBackup,
    scopedTaggerBackupDirectory,
    type TaggerBackupCategory,
    type TaggerBackupOption,
    } from "../services/taggerBackup";
    import type { RemoteLlmProfile } from "../protocol";

    type PromptMode = "tag" | "annotation";
    type OverwriteMode = "nocover" | "cover" | "merge";
    type ProcessingStage =
        | "idle"
        | "starting_backend"
        | "loading_model"
        | "tagging"
        | "annotating"
        | "complete"
        | "failed";

    type LlmFormData = {
        model: string;
        llmProvider: "local" | "open_ai" | "gemini";
        llmEndpoint: string;
        llmApiKey: string;
        llmRemoteModel: string;
        remoteConcurrency: number;
        llmProfileId: string;
        tagPrompt: string;
        annotationPrompt: string;
        overwrite: OverwriteMode;
    };

    type ModelOption = {
        label: string;
        value: string;
    };

    const path = require("node:path");
    const LLAMAFILE_SESSION_ID = "llm-main";
    const TAGGER_BACKUP_SOURCE = "eagle" as const;
    const SUPPORTED_IMAGE_EXTENSIONS = new Set([
        ".jpg",
        ".jpeg",
        ".png",
        ".webp",
        ".bmp",
    ]);

    const formData: Ref<LlmFormData> = ref({} as LlmFormData);
    const promptMode = ref<PromptMode>("tag");
    const activeBackupCategory = computed<TaggerBackupCategory>(() =>
        promptMode.value === "annotation" ? "annotations" : "tags",
    );
    const isReady = ref(false);
    const isProcessing = ref(false);
    const processingStage = ref<ProcessingStage>("idle");
    const completedItems = ref(0);
    const totalItems = ref(0);
    const showDownload = ref(false);
    const showRunnerDownload = ref(false);
    const downloadableModels = llmModelInfo.filter((item) => !item.runnerOnly);
    const modelOptions: Ref<ModelOption[]> = ref([]);
    let loadedModel = "";
    let saveTimer: ReturnType<typeof setTimeout> | undefined;
    const backups = ref<TaggerBackupOption[]>([]);
    const selectedBackup = ref("");
    const isRestoring = ref(false);
    const llmProfiles = ref<RemoteLlmProfile[]>([]);
    const loadLlmProfile = () => {
        const profile = llmProfiles.value.find(
            (item) => item.id === formData.value.llmProfileId,
        );
        if (!profile) return false;
        Object.assign(formData.value, {
            llmProvider: profile.provider,
            llmEndpoint: profile.endpoint,
            llmApiKey: profile.apiKey,
            llmRemoteModel: profile.model,
        });
        return true;
    };
    const modelSource = computed({
        get: () => formData.value.llmProvider === "local"
            ? "local"
            : `remote:${formData.value.llmProfileId}`,
        set: (value: string) => {
            if (value === "local") {
                formData.value.llmProvider = "local";
                return;
            }
            formData.value.llmProfileId = value.replace(/^remote:/, "");
            loadLlmProfile();
        },
    });
    const modelSourceOptions = computed(() => [
        { label: t.value("llm_tagger.offline"), value: "local" },
        ...llmProfiles.value.map((profile) => ({
            label: `${profile.name || profile.model || profile.endpoint} (${profile.model})`,
            value: `remote:${profile.id}`,
        })),
    ]);

    const normalizeOverwriteMode = (value: string): OverwriteMode =>
        value === "nocover" || value === "cover" || value === "merge"
            ? value
            : "merge";

    const normalizePromptMode = (value: string): PromptMode =>
        value === "annotation" ? "annotation" : "tag";

    const normalizeRemoteConcurrency = (value: unknown): number => {
        const parsed = Number(value);
        if (!Number.isFinite(parsed)) return 4;
        return Math.min(32, Math.max(1, Math.trunc(parsed)));
    };

    const refreshInstalledModels = async (preferredModel = "") => {
        const backend = getBackendClient();
        const installed = await Promise.all(
            downloadableModels.map(async (model) => {
                const files = await Promise.all(
                    model.downloadInfo.map(async (file) => {
                        const info = await backend.pathInfo(await file.dest);
                        return info.isFile;
                    }),
                );
                return files.every(Boolean)
                    ? { label: model.label, value: model.value }
                    : null;
            }),
        );
        modelOptions.value = installed.filter(
            (model): model is ModelOption => model !== null,
        );

        const preferredExists = modelOptions.value.some(
            (model) => model.value === preferredModel,
        );
        formData.value.model = preferredExists
            ? preferredModel
            : modelOptions.value[0]?.value || "";
    };

    const initializePage = async () => {
        await backenAPI.getConfig();
        const configuredModel = config.llmModelPath;
        llmProfiles.value = (config.llmRemoteProfiles || []).map((profile) => ({ ...profile }));
        if (llmProfiles.value.length === 0 && config.llmEndpoint && config.llmRemoteModel) llmProfiles.value.push({ id: "legacy", name: "远程 LLM", provider: config.llmProvider === "gemini" ? "gemini" : "open_ai", endpoint: config.llmEndpoint, apiKey: config.llmApiKey, model: config.llmRemoteModel });
        const activeProfile = llmProfiles.value.find((profile) => profile.id === config.llmRemoteProfileId) || llmProfiles.value[0];
        const useRemoteProfile = config.llmProvider !== "local" && activeProfile;
        promptMode.value = normalizePromptMode(
            config.llmTaggerOrAnnotation === "tagger"
                ? "tag"
                : config.llmTaggerOrAnnotation,
        );
        formData.value = {
            model: "",
            llmProvider: useRemoteProfile ? activeProfile.provider : "local",
            llmEndpoint: activeProfile?.endpoint || config.llmEndpoint || "",
            llmApiKey: activeProfile?.apiKey || config.llmApiKey || "",
            llmRemoteModel: activeProfile?.model || config.llmRemoteModel || "",
            remoteConcurrency: normalizeRemoteConcurrency(config.llmRemoteConcurrency),
            llmProfileId: activeProfile?.id || "",
            tagPrompt:
                config.llmTaggerPrompt || DEFAULT_LLM_TAG_PROMPT,
            annotationPrompt:
                config.llmAnnotationPrompt ||
                DEFAULT_LLM_ANNOTATION_PROMPT,
            overwrite: normalizeOverwriteMode(config.llmOverwrite),
        };
        await refreshInstalledModels(configuredModel);
        await refreshBackups();
        isReady.value = true;
        if (formData.value.model !== configuredModel) {
            await persistForm();
        }
    };

    const persistForm = async () => {
        if (!isReady.value) return;
        config.llmModelPath = formData.value.model;
        config.llmProvider = formData.value.llmProvider;
        config.llmEndpoint = formData.value.llmEndpoint;
        config.llmApiKey = formData.value.llmApiKey;
        config.llmRemoteModel = formData.value.llmRemoteModel;
        config.llmRemoteConcurrency = normalizeRemoteConcurrency(formData.value.remoteConcurrency);
        config.llmRemoteProfileId = formData.value.llmProfileId;
        config.llmTaggerPrompt = formData.value.tagPrompt;
        config.llmAnnotationPrompt = formData.value.annotationPrompt;
        config.llmOverwrite = formData.value.overwrite;
        config.llmTaggerOrAnnotation =
            promptMode.value === "tag" ? "tagger" : "annotation";
        await backenAPI.setConfig();
    };

    const schedulePersist = () => {
        if (!isReady.value) return;
        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = setTimeout(() => {
            saveTimer = undefined;
            void persistForm().catch((error) =>
                console.error("保存 LLM 配置失败", error),
            );
        }, 200);
    };

    watch(
        () => [
            formData.value.model,
            formData.value.tagPrompt,
            formData.value.annotationPrompt,
            formData.value.overwrite,
            formData.value.llmProfileId,
            formData.value.llmProvider,
            formData.value.llmEndpoint,
            formData.value.llmApiKey,
            formData.value.llmRemoteModel,
            formData.value.remoteConcurrency,
            promptMode.value,
        ],
        schedulePersist,
    );

    watch(
        () => formData.value.model,
        (model) => {
            if (loadedModel && loadedModel !== model) {
                void unloadSession();
            }
            if (isReady.value) void refreshBackups();
        },
    );

    watch(promptMode, () => {
        if (!isProcessing.value) {
            processingStage.value = "idle";
            completedItems.value = 0;
            totalItems.value = 0;
        }
        if (isReady.value) void refreshBackups();
    });

    onMounted(initializePage);
    onBeforeUnmount(() => {
        if (saveTimer) clearTimeout(saveTimer);
        void persistForm();
        void unloadSession();
    });

    const resetCurrentPrompt = () => {
        if (promptMode.value === "tag") {
            formData.value.tagPrompt = DEFAULT_LLM_TAG_PROMPT;
        } else {
            formData.value.annotationPrompt = DEFAULT_LLM_ANNOTATION_PROMPT;
        }
    };

    const changePromptMode = (mode: PromptMode) => {
        if (!isProcessing.value) {
            promptMode.value = mode;
        }
    };

    const resolveRuntimePaths = async () => {
        const model = llmModelInfo.find(
            (item) => !item.runnerOnly && item.value === formData.value.model,
        );
        if (!model) {
            throw new Error(`没有找到模型配置：${formData.value.model}`);
        }

        const files = await Promise.all(
            model.downloadInfo.map(async (file) => ({
                filename: file.filename,
                path: await file.dest,
            })),
        );
        const modelFile = files.find(
            (file) => !file.filename.toLowerCase().startsWith("mmproj"),
        );
        const mmprojFile = files.find((file) =>
            file.filename.toLowerCase().startsWith("mmproj"),
        );
        const runner = llmModelInfo.find((item) => item.runnerOnly);
        const defaultRunnerPath = runner?.downloadInfo[0]
            ? await runner.downloadInfo[0].dest
            : "";

        if (!modelFile || !mmprojFile) {
            throw new Error(`模型 ${model.label} 的文件配置不完整`);
        }

        return {
            llamafilePath: config.llmRunnerPath || defaultRunnerPath,
            modelPath: modelFile.path,
            mmprojPath: mmprojFile.path,
        };
    };

    const refreshBackups = async () => {
        try {
            backups.value = await listTaggerBackupsInDirectory(
                scopedTaggerBackupDirectory(
                    config.modelLocation,
                    TAGGER_BACKUP_SOURCE,
                    activeBackupCategory.value,
                ),
                activeBackupCategory.value,
            );
            if (!backups.value.some((backup) => backup.value === selectedBackup.value)) {
                selectedBackup.value = backups.value[0]?.value || "";
            }
        } catch {
            backups.value = [];
            selectedBackup.value = "";
        }
    };

    const restoreSelectedBackup = async () => {
        if (!selectedBackup.value || isProcessing.value || backenAPI.is_processing) return;
        isRestoring.value = true;
        try {
            const result = await restoreTaggerBackup(
                selectedBackup.value,
                (id) => eagle.item.getById(id),
                TAGGER_BACKUP_SOURCE,
            );
            notification(`已恢复 ${result.restored} 项，跳过 ${result.skipped} 项`, "success");
        } catch (error) {
            notification(error instanceof Error ? error.message : String(error), "error");
        } finally {
            isRestoring.value = false;
        }
    };

    const ensureSession = async (backend: BackendClient) => {
        const paths = await resolveRuntimePaths();
        const gpuLayers = Number.parseInt(config.llmNGL || "9999", 10);
        await backend.loadLlamafile({
            sessionId: LLAMAFILE_SESSION_ID,
            ...paths,
            port: 0,
            contextSize: Number(config.llmContextSize) || 8192,
            gpu: config.llmUseVulkan
                ? "vulkan"
                : config.llmGpu || "nvidia",
            gpuLayers: Number.isFinite(gpuLayers) ? gpuLayers : 9999,
        });
        loadedModel = formData.value.model;
    };

    const promptRunnerDownload = () => {
        dialog.warning({
            title: t.value("llm_tagger.llamafile_missing_title"),
            content: t.value("llm_tagger.llamafile_missing_notice"),
            positiveText: t.value("llm_tagger.download_llamafile"),
            negativeText: t.value("update.later"),
            onPositiveClick: () => {
                showRunnerDownload.value = true;
            },
        });
    };

    const handleRunnerDownloaded = async (paths: string[]) => {
        if (!paths[0]) return;
        config.llmRunnerPath = paths[0];
        await backenAPI.setConfig();
        notification(t.value("settings.llamafile_downloaded"));
    };

    async function unloadSession() {
        if (!loadedModel) return;
        loadedModel = "";
        await getBackendClient()
            .unloadLlamafile(LLAMAFILE_SESSION_ID)
            .catch((error) => console.error("卸载 llamafile 失败", error));
    }

    const resolveImagePath = async (
        backend: BackendClient,
        item: any,
    ): Promise<string> => {
        const candidates = [item.filePath, item.thumbnailPath].filter(
            (candidate): candidate is string => Boolean(candidate),
        );
        for (const candidate of candidates) {
            if (!SUPPORTED_IMAGE_EXTENSIONS.has(path.extname(candidate).toLowerCase())) {
                continue;
            }
            if ((await backend.pathInfo(candidate)).isFile) return candidate;
        }
        throw new Error("没有可读取的 PNG、JPEG、WebP 或 BMP 图片");
    };

    const normalizeModelContent = (content: string) =>
        content
            .replace(/<think>[\s\S]*?<\/think>/gi, "")
            .replace(/^\s*(?:标签|tags?|注释|annotation|description)\s*[:：]\s*/i, "")
            .trim();

    const parseTags = (content: string): string[] =>
        normalizeModelContent(content)
            .split(/[,，、\n|；;]/)
            .map((tag) =>
                tag
                    .replace(/^\s*[-*]?[0-9]*[.)、-]?\s*/, "")
                    .replace(/^["'“”‘’\[\]【】]+|["'“”‘’\[\]【】]+$/g, "")
                    .trim(),
            )
            .filter(Boolean);

    const writeResult = async (
        itemId: string,
        mode: PromptMode,
        content: string,
    ) => {
        const item = await eagle.item.getById(itemId);
        if (mode === "tag") {
            const generatedTags = parseTags(content);
            if (generatedTags.length === 0) {
                throw new Error("模型没有返回可用标签");
            }
            if (
                formData.value.overwrite === "nocover" &&
                (item.tags || []).length > 0
            ) {
                return;
            }
            item.tags =
                formData.value.overwrite === "cover"
                    ? generatedTags
                    : Array.from(
                          new Set([...(item.tags || []), ...generatedTags]),
                      );
        } else {
            const annotation = normalizeModelContent(content);
            if (!annotation) throw new Error("模型没有返回可用注释");
            item.annotation = annotation;
        }
        await item.save();
    };

    const processSelected = async (mode: PromptMode, targetItems?: any[], throwOnFailure = false) => {
        if (isProcessing.value || backenAPI.is_processing) {
            const message = t.value("model_download_window.process_checking");
            notification(message, "warning");
            if (throwOnFailure) throw new Error(message);
            return;
        }
        if (formData.value.llmProvider === "local" && !formData.value.model) {
            const message = t.value("llm_tagger.select_model_notice");
            notification(message, "warning");
            if (throwOnFailure) throw new Error(message);
            return;
        }

        const instruction =
            mode === "tag"
                ? formData.value.tagPrompt.trim()
                : formData.value.annotationPrompt.trim();
        if (!instruction) {
            const message = t.value("llm_tagger.prompt_required");
            notification(message, "warning");
            promptMode.value = mode;
            if (throwOnFailure) throw new Error(message);
            return;
        }

        const items = targetItems || await eagle.item.getSelected();
        if (items.length === 0) {
            const message = t.value("llm_tagger.select_images_notice");
            notification(message, "warning");
            if (throwOnFailure) throw new Error(message);
            return;
        }

        const backend = getBackendClient();
        try {
            if (formData.value.llmProvider !== "local") {
                if (!formData.value.llmEndpoint.trim() || !formData.value.llmRemoteModel.trim()) throw new Error("远程 LLM endpoint 和 model 不能为空");
            }
            const runtimePaths = formData.value.llmProvider === "local" ? await resolveRuntimePaths() : null;
            if (!runtimePaths) { await persistForm(); }
            if (!runtimePaths) { /* remote mode has no local model */ }
            if (runtimePaths) {
            const runnerInfo = await backend.pathInfo(runtimePaths.llamafilePath);
            if (!runnerInfo.isFile) {
                promptRunnerDownload();
                if (throwOnFailure) throw new Error("LLM 运行器尚未安装");
                return;
            }
            }
            await createTaggerBackupInDirectory(
                scopedTaggerBackupDirectory(
                    config.modelLocation,
                    TAGGER_BACKUP_SOURCE,
                    mode === "annotation" ? "annotations" : "tags",
                ),
                mode === "tag" ? "llm-tag" : "llm-annotation",
                items,
                TAGGER_BACKUP_SOURCE,
            );
            await refreshBackups();
        } catch (error) {
            notification(
                error instanceof Error ? error.message : String(error),
                "error",
            );
            if (throwOnFailure) throw error;
            return;
        }

        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = undefined;
        await persistForm();

        const failures: string[] = [];
        const taskId = beginTask(
            "llm",
            mode === "tag" ? "LLM 图片打标" : "LLM 写入注释",
            items.length,
        );
        if (!taskId) {
            if (throwOnFailure) throw new Error("无法启动 LLM 任务");
            return;
        }
        isProcessing.value = true;
        completedItems.value = 0;
        totalItems.value = items.length;
        processingStage.value = "starting_backend";

        try {
            await nextTick();
            backend.start();
            processingStage.value = "loading_model";
            updateTask(taskId, { detail: "正在加载模型" });
            await nextTick();
            if (formData.value.llmProvider === "local") await ensureSession(backend);
            processingStage.value =
                mode === "tag" ? "tagging" : "annotating";
            updateTask(taskId, {
                detail: mode === "tag" ? "正在打标" : "正在写入注释",
            });
            const finishItem = () => {
                completedItems.value++;
                updateTask(taskId, { completed: completedItems.value });
            };
            const failItem = (item: any, error: unknown) => {
                const message = error instanceof Error
                    ? error.message
                    : String(error);
                failures.push(`${item.name || item.id}: ${message}`);
                recordFailure({
                    taskId,
                    kind: "llm",
                    itemId: item.id,
                    name: item.name || item.id,
                    path: item.filePath || item.thumbnailPath || "",
                    error: message,
                });
                console.error("LLM 图片处理失败", item, error);
            };
            const shouldSkip = (item: any) =>
                mode === "tag" &&
                formData.value.overwrite === "nocover" &&
                (item.tags || []).length > 0;

            if (formData.value.llmProvider === "local") {
                for (const item of items) {
                    try {
                        if (shouldSkip(item)) continue;
                        const imagePath = await resolveImagePath(backend, item);
                        const result = await backend.processImageWithLlamafile({
                            sessionId: LLAMAFILE_SESSION_ID,
                            imagePath,
                            instruction,
                            model: formData.value.model.toLowerCase(),
                            temperature: Number(config.llmTemperature) || 0.5,
                            maxTokens: Number(config.llmMaxTokens) || 1024,
                            repetitionPenalty: 1.15,
                        });
                        await writeResult(item.id, mode, result.content);
                    } catch (error) {
                        failItem(item, error);
                    } finally {
                        finishItem();
                    }
                }
            } else {
                const concurrency = normalizeRemoteConcurrency(
                    formData.value.remoteConcurrency,
                );
                const remoteItems: Array<{ item: any; imagePath: string }> = [];
                for (const item of items) {
                    if (shouldSkip(item)) {
                        finishItem();
                        continue;
                    }
                    try {
                        remoteItems.push({
                            item,
                            imagePath: await resolveImagePath(backend, item),
                        });
                    } catch (error) {
                        failItem(item, error);
                        finishItem();
                    }
                }

                for (let offset = 0; offset < remoteItems.length; offset += concurrency) {
                    let pending = remoteItems.slice(offset, offset + concurrency);
                    let lastErrors = new Map<string, Error>();
                    for (let attempt = 1; attempt <= 3 && pending.length > 0; attempt++) {
                        let resultByItemId = new Map<string, { content: string; error: string }>();
                        try {
                            const batch = await backend.processBatchWithRemoteVision({
                                provider: formData.value.llmProvider,
                                endpoint: formData.value.llmEndpoint,
                                apiKey: formData.value.llmApiKey,
                                model: formData.value.llmRemoteModel,
                                images: pending.map(({ item, imagePath }) => ({
                                    itemId: item.id,
                                    imagePath,
                                })),
                                instruction,
                                temperature: Number(config.llmTemperature) || 0.5,
                                maxTokens: Number(config.llmMaxTokens) || 1024,
                                concurrency,
                            });
                            resultByItemId = new Map(
                                batch.results.map((result) => [
                                    result.itemId,
                                    {
                                        content: result.content,
                                        error: result.error,
                                    },
                                ]),
                            );
                        } catch (error) {
                            const batchError = error instanceof Error
                                ? error
                                : new Error(String(error));
                            lastErrors = new Map(
                                pending.map(({ item }) => [item.id, batchError]),
                            );
                        }

                        const retryItems: typeof pending = [];
                        for (const entry of pending) {
                            const result = resultByItemId.get(entry.item.id);
                            try {
                                if (!result) {
                                    throw lastErrors.get(entry.item.id) ||
                                        new Error("远程 LLM 未返回该图片的结果");
                                }
                                if (result.error) throw new Error(result.error);
                                await writeResult(entry.item.id, mode, result.content);
                                finishItem();
                            } catch (error) {
                                const retryError = error instanceof Error
                                    ? error
                                    : new Error(String(error));
                                lastErrors.set(entry.item.id, retryError);
                                if (attempt < 3) {
                                    retryItems.push(entry);
                                } else {
                                    failItem(
                                        entry.item,
                                        new Error(`${retryError.message}（已重试 2 次）`),
                                    );
                                    finishItem();
                                }
                            }
                        }
                        pending = retryItems;
                        if (pending.length > 0) {
                            await new Promise((resolve) =>
                                setTimeout(resolve, 500 * attempt),
                            );
                        }
                    }
                }
            }
        } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            failures.push(message);
            recordFailure({
                taskId,
                kind: "llm",
                name: "LLM 任务",
                path: "",
                error: message,
            });
        } finally {
            isProcessing.value = false;
            processingStage.value =
                failures.length > 0 ? "failed" : "complete";
            if (failures.length > 0) {
                failTask(taskId, failures.join("\n"));
            } else {
                completeTask(
                    taskId,
                    mode === "tag" ? "打标完成" : "注释完成",
                );
            }
        }

        if (failures.length > 0) {
            dialog.error({
                title: t.value("tool.failed_files_title"),
                content: () =>
                    h(
                        "div",
                        { class: "failure-list" },
                        failures.map((failure) => h("div", failure)),
                    ),
            });
        } else {
            notification(
                mode === "tag"
                    ? t.value("llm_tagger.tagging_done")
                    : t.value("llm_tagger.annotation_done"),
            );
        }
        if (throwOnFailure && failures.length > 0) throw new Error(failures.join("\n"));
    };
    defineExpose({
        ready: isReady,
        runForItems: (items: any[], mode: PromptMode) =>
            processSelected(mode, items, true),
    });
</script>

<template>
    <div v-if="isReady" class="llm-page">
        <n-form :model="formData" label-placement="left" :label-width="120">
            <n-form-item :label="t('index.progress')">
                <div class="progress-block">
                    <div class="progress-meta">
                        <n-tag size="small">
                            {{ completedItems }}/{{ totalItems }}
                        </n-tag>
                        <n-tag
                            :type="processingStage === 'failed' ? 'error' : 'info'"
                        >
                            {{ t(`llm_tagger.stage_${processingStage}`) }}
                        </n-tag>
                    </div>
                    <n-progress
                        type="line"
                        :percentage="
                            totalItems === 0
                                ? 0
                                : Number(
                                      (
                                          (completedItems / totalItems) *
                                          100
                                      ).toFixed(2),
                                  )
                        "
                        :format="(percentage: number) => `${percentage.toFixed(2)}%`"
                        indicator-placement="inside"
                        :processing="isProcessing"
                    />
                </div>
            </n-form-item>

            <n-form-item :label="t('llm_tagger.model')">
                <n-select
                    v-model:value="modelSource"
                    :options="modelSourceOptions"
                    :disabled="isProcessing"
                />
            </n-form-item>

            <n-form-item
                v-if="formData.llmProvider !== 'local'"
                label="并发"
                path="remoteConcurrency"
            >
                <n-input-number
                    v-model:value="formData.remoteConcurrency"
                    :min="1"
                    :max="32"
                    :precision="0"
                    :disabled="isProcessing"
                    style="width: 120px"
                />
            </n-form-item>

            <n-form-item
                v-if="formData.llmProvider === 'local'"
                :label="t('llm_tagger.offline')"
                path="model"
            >
                <FormHelp :content="t('llm_tagger.model_desc')" />
                <div class="model-row">
                    <n-select
                        v-model:value="formData.model"
                        :options="modelOptions"
                        :placeholder="t('llm_tagger.no_installed_models')"
                        :disabled="isProcessing"
                    />
                    <n-button
                        type="primary"
                        secondary
                        :disabled="isProcessing"
                        @click="showDownload = true"
                    >
                        <template #icon>
                            <n-icon><CloudDownloadOutline /></n-icon>
                        </template>
                        {{ t("index.model_download_window") }}
                    </n-button>
                </div>
            </n-form-item>

            <n-form-item
                :label="promptMode === 'annotation' ? '注释备份' : '标签备份'"
            >
                <n-select
                    v-model:value="selectedBackup"
                    :options="backups"
                    clearable
                    :placeholder="
                        promptMode === 'annotation'
                            ? '选择要恢复的注释备份'
                            : '选择要恢复的标签备份'
                    "
                    :disabled="isProcessing || isRestoring || backups.length === 0"
                />
                <n-button
                    secondary
                    :loading="isRestoring"
                    :disabled="!selectedBackup || isProcessing"
                    @click="restoreSelectedBackup"
                >
                    {{ promptMode === "annotation" ? "恢复注释" : "恢复标签" }}
                </n-button>
            </n-form-item>

            <n-form-item :label="t('llm_tagger.prompt')">
                <FormHelp :content="t('llm_tagger.prompt_desc')" />
                <div class="prompt-editor">
                    <n-tabs
                        :value="promptMode"
                        type="segment"
                        @update:value="changePromptMode"
                    >
                        <n-tab-pane
                            name="tag"
                            :tab="t('llm_tagger.tag_prompt')"
                            :disabled="isProcessing"
                        >
                            <n-input
                                v-model:value="formData.tagPrompt"
                                type="textarea"
                                :autosize="{ minRows: 10, maxRows: 18 }"
                                :disabled="isProcessing"
                            />
                        </n-tab-pane>
                        <n-tab-pane
                            name="annotation"
                            :tab="t('llm_tagger.annotation_prompt')"
                            :disabled="isProcessing"
                        >
                            <n-input
                                v-model:value="formData.annotationPrompt"
                                type="textarea"
                                :autosize="{ minRows: 10, maxRows: 18 }"
                                :disabled="isProcessing"
                            />
                        </n-tab-pane>
                    </n-tabs>
                </div>
            </n-form-item>

            <n-form-item
                v-if="promptMode === 'tag'"
                :label="t('index.is_overwrite')"
                path="overwrite"
            >
                <FormHelp :content="t('index.is_overwrite_desc')" html />
                <n-radio-group
                    v-model:value="formData.overwrite"
                    name="llmOverwrite"
                    :disabled="isProcessing"
                >
                    <n-space>
                        <n-radio value="nocover">
                            {{ t("index.no_cover") }}
                        </n-radio>
                        <n-radio value="cover">
                            {{ t("index.cover") }}
                        </n-radio>
                        <n-radio value="merge">
                            {{ t("index.merge") }}
                        </n-radio>
                    </n-space>
                </n-radio-group>
            </n-form-item>

            <div class="action-row">
                <n-button
                    :disabled="isProcessing"
                    @click="resetCurrentPrompt"
                >
                    {{
                        promptMode === "tag"
                            ? t("llm_tagger.reset_tag_prompt")
                            : t("llm_tagger.reset_annotation_prompt")
                    }}
                </n-button>
                <n-button
                    type="primary"
                    :disabled="isProcessing || (formData.llmProvider === 'local' ? !formData.model : (!formData.llmEndpoint || !formData.llmRemoteModel))"
                    :loading="isProcessing"
                    @click="processSelected(promptMode)"
                >
                    {{
                        promptMode === "tag"
                            ? t("llm_tagger.tag_selected")
                            : t("llm_tagger.annotate_selected")
                    }}
                </n-button>
            </div>
        </n-form>
    </div>

    <downloadModal
        v-model:showModal="showDownload"
        model-type="llm"
        :initial-selection="formData.model"
        :reload-on-complete="false"
        @completed="refreshInstalledModels(formData.model)"
    />
    <downloadModal
        v-model:showModal="showRunnerDownload"
        model-type="llm"
        runner-only
        :reload-on-complete="false"
        @completed="handleRunnerDownloaded"
    />
</template>

<style scoped>
    .llm-page {
        width: min(900px, calc(100% - 48px));
        margin: 30px auto 0;
        padding-bottom: 40px;
    }

    .llm-page :deep(.n-form-item-blank) {
        min-width: 0;
        gap: 10px;
    }

    .llm-page :deep(.n-form-item-blank > .n-input),
    .llm-page :deep(.n-form-item-blank > .n-input-number),
    .llm-page :deep(.n-form-item-blank > .n-select) {
        min-width: 0;
        flex: 1 1 0;
    }

    .model-row {
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto;
        align-items: center;
        gap: 12px;
        width: 100%;
    }

    .progress-block {
        display: grid;
        gap: 8px;
        width: 100%;
    }

    .progress-meta {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
    }

    .prompt-editor {
        width: 100%;
        min-width: 0;
    }

    .action-row {
        display: flex;
        align-items: center;
        gap: 10px;
        justify-content: space-between;
        flex-wrap: wrap;
    }

    .failure-list {
        display: grid;
        gap: 6px;
        color: #d9534f;
        white-space: pre-wrap;
    }

    @media (max-width: 720px) {
        .llm-page {
            width: calc(100% - 24px);
            margin-top: 18px;
        }

        .action-row {
            align-items: stretch;
            flex-direction: column;
        }

    }
</style>
