import {
    type NotificationType,
    createDiscreteApi,
    darkTheme,
} from "naive-ui";
import { h, ref } from "vue";
import type { Config, WdModelKind, WdTagScore } from "../protocol";
import { getBackendClient } from "../services/backendClient";
import {
    activeTask,
    isTaskRunning,
    recordFailure,
} from "../services/taskCenter";

const path = require("node:path");
let error_image: string[] = [];

const notify = createDiscreteApi(["notification"], {
    notificationProviderProps: {
        max: 3,
    },
    configProviderProps: {
        theme: darkTheme,
    },
}).notification;
export const dialog = createDiscreteApi(["dialog"], {
    configProviderProps: {
        theme: darkTheme,
    },
}).dialog;
export const notification = (
    message: string,
    type: NotificationType = "info",
) => {
    notify[type]({
        content: message,
        duration: 3000,
    });
};

export let config = {} as Config;
export const completeItem = ref(0);
export const t = ref((key: any) => i18next.t(key));
export const convertPath = (filePath: string) => {
    const pluginPath = window.pluginPath;
    if (import.meta.env.DEV) {
        if (pluginPath) {
            return path.join(pluginPath, "..", filePath);
        }
        return path.resolve(filePath);
    }
    filePath = filePath.replace("src/public/", "");
    if (pluginPath) {
        return path.join(pluginPath, filePath);
    }
    return path.resolve(filePath);
};
export const backenAPI = {
    get is_processing() {
        return isTaskRunning.value;
    },
    async getConfig() {
        const result = await getBackendClient().readConfig();
        config = result.config;
        return config;
    },

    async setConfig() {
        return getBackendClient().writeConfig(config);
    },

    async checkForUpdate() {
        return getBackendClient().checkForUpdate();
    },
    async getTargetItems(isAll: boolean = false) {
        let items = await eagle.item.getSelected();
        if (items.length === 0 || isAll) {
            items = await eagle.item.getAll();
        }
        return items;
    },
    async initialize(isAll: boolean = false) {
        error_image = [];
        return this.getTargetItems(isAll);
    },
    filterItem(items: any) {
        // 过滤
        return items.map((item: any) => ({
            id: item.id,
            name: item.name,
            ext: item.ext,
            tags: item.tags,
            filePath: item.filePath,
            thumbnailPath: item.thumbnailPath,
        }));
    },
    async startGetTag(items: any) {
        error_image = [];
        await this.getConfig();
        return this.startWdTagger(items);
    },
    async startWdTagger(items: any) {
        const itemsMap = this.filterItem(items);
        const backend = getBackendClient();
        const batchSize = Math.max(1, Number(config.steps) || 1);
        const sessionId = "wd-main";
        const model = await this.resolveWdModel(backend);
        const videoExtensions = new Set([
            "mp4",
            "avi",
            "mov",
            "mkv",
            "flv",
            "wmv",
            "webm",
        ]);
        const imageExtensions = new Set([
            "png",
            "jpg",
            "jpeg",
            "webp",
            "bmp",
        ]);
        const videos = itemsMap.filter((item: any) =>
            videoExtensions.has(String(item.ext).toLowerCase()),
        );
        const images = itemsMap.filter(
            (item: any) =>
                imageExtensions.has(String(item.ext).toLowerCase()),
        );
        const supportedIds = new Set(
            [...videos, ...images].map((item: any) => item.id),
        );
        const unsupported = itemsMap.filter(
            (item: any) => !supportedIds.has(item.id),
        );
        completeItem.value += unsupported.length;
        error_image.push(
            ...unsupported.map(
                (item: any) =>
                    item.filePath || item.thumbnailPath || item.name || item.id,
            ),
        );
        for (const item of unsupported) {
            recordFailure({
                taskId: activeTask.value?.id || "",
                kind: "wd",
                itemId: item.id,
                name: item.name || item.id,
                path: item.filePath || item.thumbnailPath || "",
                error: "不支持的文件类型",
            });
        }

        notification(
            `${t.value("tool.tagger_prefix")}${itemsMap.length}${t.value("tool.tagger_suffix")}`,
        );

        try {
            await backend.loadWdTagger(
                sessionId,
                model.modelPath,
                model.tagsPath,
                model.kind,
                "auto",
                convertPath("tagset.csv"),
                config.language === "zh" || config.language === "mix"
                    ? config.language
                    : "en",
                config.splitter,
                [...config.filterTags],
            );

            if (config.readVideo === "read") {
                const ffmpegPaths =
                    await eagle.extraModule.ffmpeg.getPaths();
                for (const video of videos) {
                    if (this.shouldSkipWdItem(video)) {
                        completeItem.value++;
                        continue;
                    }
                    try {
                        const result = await backend.tagVideoWithWdTagger(
                            sessionId,
                            video.filePath,
                            ffmpegPaths.ffmpeg,
                            ffmpegPaths.ffprobe,
                            6,
                            batchSize,
                            Number(config.threshold),
                        );
                        await this.saveWdTags(video.id, result.tags);
                    } catch (error) {
                        error_image.push(video.filePath);
                        recordFailure({
                            taskId: activeTask.value?.id || "",
                            kind: "wd",
                            itemId: video.id,
                            name: video.name || video.id,
                            path: video.filePath,
                            error: error instanceof Error ? error.message : String(error),
                        });
                        console.error("视频打标失败", video.filePath, error);
                    }
                }
            } else {
                await this.tagWdImages(
                    backend,
                    sessionId,
                    videos,
                    batchSize,
                );
            }

            await this.tagWdImages(
                backend,
                sessionId,
                images,
                batchSize,
            );
        } catch (error) {
            console.error("WD 打标失败", error);
            error_image.push(
                error instanceof Error ? error.message : String(error),
            );
            recordFailure({
                taskId: activeTask.value?.id || "",
                kind: "wd",
                name: "WD 打标任务",
                path: "",
                error: error instanceof Error ? error.message : String(error),
            });
        } finally {
            await backend.unloadWdTagger(sessionId).catch((error) => {
                console.error("卸载 WD session 失败", error);
            });
            this.showTaggingResult();
        }
        return { failureCount: error_image.length };
    },
    async resolveWdModel(backend: ReturnType<typeof getBackendClient>): Promise<{
        kind: WdModelKind;
        modelPath: string;
        tagsPath: string;
    }> {
        const result = await backend.scanWdModels(config.modelLocation);
        const model = result.models.find(
            (candidate) => candidate.name === config.modelPath,
        );
        if (!model) {
            throw new Error(
                `没有在 ${result.modelsDirectory} 找到模型 ${config.modelPath}`,
            );
        }
        return {
            kind: model.modelKind,
            modelPath: model.modelPath,
            tagsPath: model.tagsPath,
        };
    },
    shouldSkipWdItem(item: any) {
        return config.overwrite === "nocover" && item.tags.length > 0;
    },
    async tagWdImages(
        backend: ReturnType<typeof getBackendClient>,
        sessionId: string,
        items: any[],
        batchSize: number,
    ) {
        let queued = 0;
        for (const item of items) {
            if (this.shouldSkipWdItem(item)) {
                completeItem.value++;
                continue;
            }
            await backend.enqueueWdTaggerImage(sessionId, {
                id: item.id,
                path: item.thumbnailPath,
            });
            queued++;

            if (queued === batchSize) {
                await this.completeWdImageBatch(backend, sessionId);
                queued = 0;
            }
        }
        if (queued > 0) {
            await this.completeWdImageBatch(backend, sessionId);
        }
    },
    async completeWdImageBatch(
        backend: ReturnType<typeof getBackendClient>,
        sessionId: string,
    ) {
        const result = await backend.completeWdTaggerBatch(
            sessionId,
            Number(config.threshold),
        );
        const failedIds = new Set(result.failures.map((failure) => failure.id));
        for (const failure of result.failures) {
            error_image.push(failure.path);
            recordFailure({
                taskId: activeTask.value?.id || "",
                kind: "wd",
                itemId: failure.id,
                name: failure.id,
                path: failure.path,
                error: failure.error,
            });
            completeItem.value++;
            console.error("图片打标失败", failure.path, failure.error);
        }
        for (const prediction of result.predictions) {
            if (failedIds.has(prediction.id)) continue;
            try {
                await this.saveWdTags(prediction.id, prediction.tags);
            } catch (error) {
                error_image.push(prediction.path);
                recordFailure({
                    taskId: activeTask.value?.id || "",
                    kind: "wd",
                    itemId: prediction.id,
                    name: prediction.id,
                    path: prediction.path,
                    error: error instanceof Error ? error.message : String(error),
                });
                completeItem.value++;
                console.error("写入图片标签失败", prediction.path, error);
            }
        }
    },
    async saveWdTags(itemId: string, scores: WdTagScore[]) {
        const tags = scores.map((tag) => tag.name);
        const item = await eagle.item.getById(itemId);
        item.tags =
            config.overwrite === "merge"
                ? Array.from(new Set([...item.tags, ...tags]))
                : tags;
        await item.save();
        completeItem.value++;
    },
    showTaggingResult() {
        if (error_image.length > 0) {
            dialog.error({
                title: t.value("tool.failed_files_title"),
                content: () =>
                    h("div", [
                        h("div", t.value("tool.failed_files")),
                        ...error_image.map((fileName) =>
                            h("div", { style: "color: red" }, fileName),
                        ),
                    ]),
            });
            return;
        }
        notification(
            `${t.value("tool.total_success_prefix")}${completeItem.value}${t.value("tool.total_success_suffix")}`,
            "info",
        );
    },
    async checkFileExists(filePath: string) {
        return (await getBackendClient().pathInfo(filePath)).exists;
    },
    async saveLlamafileRunnerPath(runnerPath: string) {
        const pathInfo = await getBackendClient().pathInfo(runnerPath);
        if (!pathInfo.isFile) {
            throw new Error("没有找到 llamafile runner");
        }
        await this.getConfig();
        config.llmRunnerPath = runnerPath;
        await this.setConfig();
        return runnerPath;
    },
};
