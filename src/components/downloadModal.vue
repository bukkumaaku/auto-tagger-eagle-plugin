<script lang="ts" setup>
    import {
        NCard,
        NTag,
        NSpin,
        NCollapseTransition,
        NModal,
        NRadioButton,
        NProgress,
        NButton,
        NSpace,
        NRadioGroup,
        NSelect,
        type ProgressStatus,
        useDialog,
    } from "naive-ui";
    import { computed, ref, watch, type Ref } from "vue";
    import type { DownloadOptions } from "../api/interface";
    import {
        embeddingModelInfo,
        llmModelInfo,
        wdModelInfo,
    } from "../api/modelInfo";
    import { t } from "../api/backen";
    import { getBackendClient } from "../services/backendClient";
    import {
        beginTask,
        completeTask,
        failTask,
        recordFailure,
        updateTask,
    } from "../services/taskCenter";
    import FormHelp from "./formHelp.vue";
    const dialog = useDialog();
    const divideByte = (byte: number) => {
        if (byte < 1024) {
            return byte + "B";
        } else if (byte < 1024 * 1024) {
            return (byte / 1024).toFixed(2) + "KB";
        } else if (byte < 1024 * 1024 * 1024) {
            return (byte / 1024 / 1024).toFixed(2) + "MB";
        } else {
            return (byte / 1024 / 1024 / 1024).toFixed(2) + "GB";
        }
    };
    const props = withDefaults(
        defineProps<{
            modelType?: "wd" | "llm" | "embedding";
            runnerOnly?: boolean;
            destinationDirectory?: string;
            reloadOnComplete?: boolean;
            initialSelection?: string;
        }>(),
        {
            modelType: "wd",
            runnerOnly: false,
            destinationDirectory: "",
            reloadOnComplete: true,
            initialSelection: "",
        },
    );
    const emit = defineEmits<{
        completed: [paths: string[]];
    }>();
    const showModal = defineModel<boolean>("showModal");
    const path = require("node:path");

    const downloadSource = ref("mirror");
    const isDownloading = ref(false);
    const selectedModel = ref("");
    const modelInfoByType = {
        wd: wdModelInfo,
        llm: llmModelInfo,
        embedding: embeddingModelInfo,
    };
    const modelOptions = computed(() => {
        const options = modelInfoByType[props.modelType];
        return props.runnerOnly
            ? options.filter((option: any) => option.runnerOnly === true)
            : options.filter((option: any) => option.runnerOnly !== true);
    });
    const selectedDownloadInfo = computed(
        () =>
            modelOptions.value.find(
                (option) => option.value === selectedModel.value,
            )?.downloadInfo ?? [],
    );
    const hasHuggingFaceFiles = computed(() =>
        selectedDownloadInfo.value.some((item) =>
            /^https:\/\/(?:www\.)?huggingface\.co\//i.test(item.url),
        ),
    );
    const hasDownloadFailures = computed(() =>
        downloadItems.value.some((item) => item.status === "error"),
    );
    watch(
        modelOptions,
        (options) => {
            selectedModel.value = options[0]?.value || "";
        },
        { immediate: true },
    );
    watch(showModal, (visible) => {
        if (
            visible &&
            props.initialSelection &&
            modelOptions.value.some(
                (option) => option.value === props.initialSelection,
            )
        ) {
            selectedModel.value = props.initialSelection;
        }
    });
    const handleCloseModal = () => {
        showModal.value = false;
    };
    const basicDownloadInfo = {
        percentage: 0,
        processing: true,
        status: "info" as ProgressStatus,
        errorText: "",
        downloadedBytes: 0,
        totalBytes: 0,
        realTimeSpeed: "0",
    };
    const downloadItems: Ref<DownloadOptions[]> = ref([]);
    const startDownload = async () => {
        const downloadFileIndex = modelOptions.value.findIndex(
            (item) => item.value === selectedModel.value,
        );
        if (downloadFileIndex === -1) {
            return;
        }
        const backend = getBackendClient();
        isDownloading.value = true;
        downloadItems.value = [];
        try {
            for (const item of modelOptions.value[downloadFileIndex]
                .downloadInfo) {
                const realDestPath = props.destinationDirectory
                    ? path.join(props.destinationDirectory, item.filename)
                    : await item.dest;

                downloadItems.value.push({
                    ...basicDownloadInfo,
                    ...item,
                    dest: realDestPath,
                });
            }
        } catch (error) {
            isDownloading.value = false;
            dialog.error({
                title: t.value("model_download_window.download_failed"),
                content: error instanceof Error ? error.message : String(error),
                positiveText: "OK",
            });
            return;
        }
        const taskId = beginTask("download", "模型下载");
        if (!taskId) {
            isDownloading.value = false;
            dialog.warning({
                title: "任务正在运行",
                content: "请等待当前任务完成后再下载。",
                positiveText: "OK",
            });
            return;
        }
        const totalBytes = () =>
            downloadItems.value.reduce(
                (sum, item) => sum + item.totalBytes,
                0,
            );
        const downloadedBytes = () =>
            downloadItems.value.reduce(
                (sum, item) => sum + item.downloadedBytes,
                0,
            );
        let failed = false;
        for (const item of downloadItems.value) {
            const url =
                downloadSource.value === "mirror" &&
                /^https:\/\/(?:www\.)?huggingface\.co\//i.test(item.url)
                    ? item.url.replace(
                          "https://huggingface.co/",
                          "https://hf-mirror.com/",
                      )
                    : item.url;
            try {
                const result = await backend.downloadFile(
                    url,
                    item.dest,
                    (progress) => {
                        item.percentage = Number(
                            (progress.percentage ?? 0).toFixed(2),
                        );
                        item.realTimeSpeed = (
                            progress.bytesPerSecond /
                            (1024 * 1024)
                        ).toFixed(2);
                        item.downloadedBytes = progress.downloadedBytes;
                        item.totalBytes = progress.totalBytes ?? 0;
                        updateTask(taskId, {
                            detail: `正在下载 ${item.filename}`,
                            completed: downloadedBytes(),
                            total: totalBytes(),
                        });
                    },
                );
                item.percentage = 100;
                item.downloadedBytes = result.downloadedBytes;
                item.totalBytes =
                    result.totalBytes ?? result.downloadedBytes;
                item.realTimeSpeed = "0";
                item.status = "success";
                item.processing = false;
            } catch (error) {
                failed = true;
                item.status = "error";
                item.errorText =
                    error instanceof Error ? error.message : String(error);
                recordFailure({
                    taskId,
                    kind: "download",
                    name: item.filename,
                    path: item.dest,
                    error: item.errorText,
                });
                item.processing = false;
            }
        }
        isDownloading.value = false;
        if (failed) {
            const errors = downloadItems.value
                .filter((item) => item.errorText)
                .map((item) => `${item.filename}: ${item.errorText}`)
                .join("\n");
            dialog.error({
                title: t.value("model_download_window.download_failed"),
                content:
                    errors ||
                    t.value("model_download_window.download_failed_notice"),
                positiveText: "OK",
            });
            failTask(taskId, errors || "部分文件下载失败");
        } else {
            completeTask(taskId, "下载完成");
            await finishDownload(
                downloadItems.value.map((item) => item.dest),
            );
        }
    };
    const formatPercentage = (percentage: number) =>
        `${percentage.toFixed(2)}%`;
    const finishDownload = async (paths: string[]) => {
        emit("completed", paths);
        const finish = () => {
            showModal.value = false;
            if (props.reloadOnComplete) globalThis.location.reload();
        };
        dialog.success({
            title: t.value("model_download_window.all_done"),
            content: t.value("model_download_window.all_done_notice"),
            positiveText: "OK",
            onPositiveClick: finish,
            onClose: finish,
            onMaskClick: finish,
        });
    };
</script>

<template>
    <n-modal
        v-model:show="showModal"
        preset="card"
        :title="t('model_download_window.title')"
        :mask-closable="!isDownloading"
        :close-on-esc="!isDownloading"
        :closable="!isDownloading"
        :on-close="handleCloseModal"
        style="width: 85%"
    >
        <n-space vertical size="large">
            <n-card embedded :bordered="false" size="small">
                <n-space vertical>
                    <div class="setting-row">
                        <span class="label"
                            >{{
                                t("model_download_window.select_model")
                            }}：</span
                        >
                        <FormHelp
                            :content="t('model_download_window.select_model_desc')"
                        />
                        <n-select
                            v-model:value="selectedModel"
                            :options="modelOptions"
                            :disabled="isDownloading"
                            :placeholder="
                                t(
                                    'model_download_window.select_model_placeholder',
                                )
                            "
                            style="width: 300px"
                        />
                    </div>

                    <div v-if="hasHuggingFaceFiles" class="setting-row">
                        <span class="label"
                            >{{
                                t("model_download_window.download_source")
                            }}：</span
                        >
                        <FormHelp
                            :content="t('model_download_window.download_source_desc')"
                        />
                        <n-radio-group
                            v-model:value="downloadSource"
                            :disabled="isDownloading"
                        >
                            <n-radio-button value="direct">
                                {{
                                    t(
                                        "model_download_window.huggingface_direct",
                                    )
                                }}
                            </n-radio-button>
                            <n-radio-button value="mirror">
                                {{
                                    t(
                                        "model_download_window.huggingface_mirror",
                                    )
                                }}
                            </n-radio-button>
                        </n-radio-group>
                    </div>
                </n-space>
            </n-card>

            <div
                v-if="!isDownloading"
                style="display: flex; justify-content: flex-end"
            >
                <n-button
                    type="primary"
                    size="large"
                    @click="startDownload"
                    :disabled="!selectedModel"
                >
                    {{ t("model_download_window.start_download") }}
                </n-button>
            </div>

            <n-collapse-transition
                :show="isDownloading || downloadItems.length > 0"
            >
                <n-card
                    title="下载进度"
                    embedded
                    :bordered="false"
                    size="small"
                >
                    <template #header-extra> </template>

                    <n-space vertical size="medium">
                        <div
                            class="progress-item"
                            v-for="item in downloadItems"
                        >
                            <n-space class="progress-label">
                                <span>{{ item.filename }}</span>

                                <n-tag type="info" size="small" round>
                                    {{ t("model_download_window.speed") }}:{{
                                        item.realTimeSpeed
                                    }}MB/s
                                </n-tag>
                                <n-tag size="small" round>
                                    {{ divideByte(item.downloadedBytes) }}/
                                    {{ divideByte(item.totalBytes) }}
                                </n-tag>
                                <n-tag
                                    size="small"
                                    round
                                    type="error"
                                    v-if="item.errorText"
                                >
                                    {{ item.errorText }}
                                </n-tag>
                            </n-space>
                            <n-progress
                                type="line"
                                :percentage="item.percentage"
                                :format="formatPercentage"
                                :indicator-placement="'inside'"
                                :processing="item.processing"
                                :status="item.status"
                            />
                        </div>
                    </n-space>

                    <div class="download-footer">
                        <n-spin size="small" v-if="isDownloading" />
                        <span style="margin-left: 8px">
                            {{
                                isDownloading
                                    ? t(
                                          "model_download_window.downloading_notice",
                                      )
                                    : hasDownloadFailures
                                      ? t(
                                            "model_download_window.download_failed_notice",
                                        )
                                    : t(
                                          "model_download_window.download_success",
                                      )
                            }}
                        </span>
                    </div>
                </n-card>
            </n-collapse-transition>
        </n-space>
    </n-modal>
</template>
