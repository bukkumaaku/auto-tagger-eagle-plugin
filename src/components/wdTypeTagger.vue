<script setup lang="ts">
    import {
        NThing,
        NButton,
        NSpace,
        NProgress,
        NForm,
        NFormItem,
        NRadio,
        NRadioGroup,
        NSelect,
        NInput,
        NInputNumber,
        NIcon,
        NTag,
    } from "naive-ui";
    import { CloudDownloadOutline } from "@vicons/ionicons5";
    import { onBeforeUnmount, onMounted, ref, watch, type Ref } from "vue";
    import {
        backenAPI,
        notification,
        t,
        completeItem,
        config,
    } from "../api/backen";
    import downloadModal from "./downloadModal.vue";
    import FormHelp from "./formHelp.vue";
    import { getBackendClient } from "../services/backendClient";
    import {
        beginTask,
        completeTask,
        failTask,
        updateTask,
    } from "../services/taskCenter";

    type WdFormData = {
        modelPath: string;
        threshold: number;
        steps: number;
        filterTags: string[];
        overwrite: string;
        readVideo: string;
        language: string;
        splitter: string;
    };

    const disableVideoRead = ref(true);
    const cloneFormData = (value: WdFormData): WdFormData => ({
        ...value,
        filterTags: [...value.filterTags],
    });
    const formData: Ref<WdFormData> = ref({} as WdFormData);
    let originalConfig: WdFormData | null = null;
    const isReady = ref(false);
    const isTagging = ref(false);
    const taskId = ref("");
    let refreshTimer: ReturnType<typeof setInterval> | undefined;
    let isRefreshing = false;

    const refreshItemCount = async () => {
        if (isTagging.value || isRefreshing) return;
        isRefreshing = true;
        try {
            const items = await backenAPI.getTargetItems();
            allItem.value = items.length;
            completeItem.value = 0;
        } catch (error) {
            console.error("刷新待打标数量失败", error);
        } finally {
            isRefreshing = false;
        }
    };

    const startRefreshTimer = () => {
        if (refreshTimer || isTagging.value) return;
        refreshTimer = setInterval(() => void refreshItemCount(), 500);
    };

    const stopRefreshTimer = () => {
        if (refreshTimer) clearInterval(refreshTimer);
        refreshTimer = undefined;
    };

    const config2FormData = () => {
        formData.value = {
            modelPath: config.modelPath,
            threshold: config.threshold,
            steps: config.steps,
            filterTags: [...config.filterTags],
            overwrite: config.overwrite,
            readVideo: config.readVideo,
            language: config.language,
            splitter: config.splitter,
        };
    };

    const formData2Config = () => {
        config.modelPath = formData.value.modelPath;
        config.threshold = formData.value.threshold;
        config.steps = formData.value.steps;
        config.filterTags = [...formData.value.filterTags];
        config.overwrite = formData.value.overwrite;
        config.readVideo = formData.value.readVideo;
        config.language = formData.value.language;
        config.splitter = formData.value.splitter;
    };

    const initializePage = async () => {
        await backenAPI.getConfig();
        config2FormData();
        if (config.modelLocation) {
            try {
                const result = await getBackendClient().scanWdModels(
                    config.modelLocation,
                );
                options.value = result.models.map((model) => ({
                    label: model.name,
                    value: model.name,
                }));
                if (
                    options.value.length > 0 &&
                    !options.value.some(
                        (option) => option.value === formData.value.modelPath,
                    )
                ) {
                    formData.value.modelPath = options.value[0].value;
                    config.modelPath = formData.value.modelPath;
                    await backenAPI.setConfig();
                }
            } catch (error) {
                options.value = [];
                notification(
                    error instanceof Error ? error.message : String(error),
                    "error",
                );
            }
        }
        originalConfig = cloneFormData(formData.value);
        isReady.value = true;
        const items = await backenAPI.initialize();
        allItem.value = items.length;
        disableVideoRead.value = await eagle.extraModule.ffmpeg.isInstalled();
        startRefreshTimer();
    };

    onMounted(initializePage);
    onBeforeUnmount(stopRefreshTimer);
    // 提交处理
    const handleSubmit = async (isAll = false) => {
        if (formData.value?.modelPath === "") {
            notification(
                t.value("model_download_window.select_model_location_notice"),
            );
            return;
        }
        if (isTagging.value || backenAPI.is_processing) {
            notification(t.value("model_download_window.process_checking"));
            return;
        }
        const startedTask = beginTask("wd", "WD 批量打标");
        if (!startedTask) return;
        taskId.value = startedTask;
        isTagging.value = true;
        stopRefreshTimer();
        try {
            formData2Config();
            await backenAPI.setConfig();
            const items = await backenAPI.initialize(isAll);
            allItem.value = items.length;
            completeItem.value = 0;
            updateTask(startedTask, {
                detail: "正在加载模型",
                total: items.length,
            });
            const result = await backenAPI.startGetTag(items);
            completeTask(
                startedTask,
                result.failureCount > 0
                    ? `完成，${result.failureCount} 个项目失败`
                    : "打标完成",
            );
        } catch (error) {
            failTask(startedTask, error);
            notification(
                error instanceof Error ? error.message : String(error),
                "error",
            );
        } finally {
            taskId.value = "";
            isTagging.value = false;
            await refreshItemCount();
            startRefreshTimer();
        }
    };
    // 重置表单
    const handleReset = () => {
        if (!originalConfig) return;
        formData.value = cloneFormData(originalConfig);
        formData2Config();
    };
    const options: Ref<Array<{ label: string; value: string }>> = ref([]);
    watch(
        formData,
        () => {
            if (isReady.value) {
                formData2Config();
                void backenAPI.setConfig().catch((error) =>
                    console.error("保存 WD 配置失败", error),
                );
            }
        },
        { deep: true },
    );
    const allItem = ref(0);
    watch([completeItem, allItem], ([completed, total]) => {
        if (taskId.value) {
            updateTask(taskId.value, {
                detail: "正在写入标签",
                completed,
                total,
            });
        }
    });
    const showModal = ref(false);
</script>

<template>
    <n-form
        v-if="isReady"
        :model="formData"
        label-placement="left"
        label-width="auto"
        style="
            width: 75% !important;
            margin: auto auto;
            position: relative;
            margin-top: 30px;
        "
    >
        <n-form-item :label="t('index.progress')" path="modelName">
            <n-thing>{{ completeItem }}/{{ allItem }}</n-thing>
            &nbsp;&nbsp;
            <n-progress
                type="line"
                :percentage="
                    allItem === 0
                        ? 0
                        : Number(((completeItem / allItem) * 100).toFixed(2))
                "
                indicator-placement="inside"
            />
        </n-form-item>
        <n-form-item :label="t('index.model_location')" path="modelPath">
            <FormHelp :content="t('index.model_location_desc')" />
            <n-select
                v-model:value="formData.modelPath"
                :options="options"
                style="width: 70%"
            ></n-select>
            <n-button
                type="primary"
                secondary
                :disabled="isTagging"
                @click="showModal = true"
                style="margin: auto; margin-right: 0px"
            >
                <template #icon>
                    <n-icon><CloudDownloadOutline /></n-icon>
                </template>
                {{ t("index.model_download_window") }}
            </n-button>
        </n-form-item>
        <n-form-item :label="t('index.threthold')" path="threshold">
            <FormHelp :content="t('index.threshold_desc')" html />
            <n-input-number
                v-model:value="formData.threshold"
                :min="0"
                :max="1"
                :step="0.01"
            />
        </n-form-item>
        <n-form-item :label="t('index.batch_size')" path="steps">
            <FormHelp :content="t('index.batch_size_desc')" html />
            <n-input-number v-model:value="formData.steps" :min="1" :max="40" />
        </n-form-item>
        <n-form-item :label="t('index.filter_tags')" path="filterTags">
            <FormHelp :content="t('index.filter_tags_desc')" html />
            <n-select
                filterable
                multiple
                tag
                :show="false"
                :show-arrow="false"
                v-model:value="formData.filterTags"
                :placeholder="t('index.filter_tags_palceholder')"
            />
        </n-form-item>
        <n-form-item :label="t('index.is_overwrite')" path="overwrite">
            <FormHelp :content="t('index.is_overwrite_desc')" html />
            <n-radio-group v-model:value="formData.overwrite" name="overwrite">
                <n-space>
                    <n-radio value="nocover">{{ t("index.no_cover") }}</n-radio>
                    <n-radio value="cover">{{ t("index.cover") }}</n-radio>
                    <n-radio value="merge">{{ t("index.merge") }}</n-radio>
                </n-space>
            </n-radio-group>
        </n-form-item>
        <n-form-item :label="t('index.read_video')" path="readVideo">
            <FormHelp :content="t('index.read_video_desc')" html />
            <n-radio-group v-model:value="formData.readVideo" name="readVideo">
                <n-space>
                    <n-radio value="noread">{{
                        t("index.no_read_video")
                    }}</n-radio>
                    <n-radio value="read" :disabled="!disableVideoRead">
                        {{ t("index.read_video_content") }}
                        <n-tag type="warning" v-if="!disableVideoRead"
                            >install ffmpeg to enable
                        </n-tag>
                    </n-radio>
                </n-space>
            </n-radio-group>
        </n-form-item>
        <n-form-item :label="t('index.lable_language')" path="language">
            <FormHelp :content="t('index.label_language_desc')" />
            <n-radio-group v-model:value="formData.language" name="language">
                <n-space>
                    <n-radio value="zh">{{ t("index.chinese") }}</n-radio>
                    <n-radio value="en">{{ t("index.english") }}</n-radio>
                    <n-radio value="mix">{{ t("index.language_mix") }}</n-radio>
                </n-space>
            </n-radio-group>
        </n-form-item>
        <n-form-item :label="t('index.splitter')" path="splitter">
            <FormHelp :content="t('index.splitter_desc')" html />
            <n-input
                v-model:value="formData.splitter"
                :placeholder="
                    formData.language === 'mix'
                        ? t('index.splitter_palceholder_no_mix')
                        : t('index.splitter_palceholder_mix')
                "
                :disabled="formData.language !== 'mix'"
            />
        </n-form-item>
        <div style="display: flex; flex-direction: column; gap: 10px">
            <div style="display: flex; gap: 10px; justify-content: flex-end">
                <n-button
                    type="primary"
                    :disabled="isTagging"
                    :loading="isTagging"
                    @click="handleSubmit(false)"
                >
                    {{ t("index.confirm") }}
                </n-button>
                <n-button @click="handleReset">{{ t("index.reset") }}</n-button>
            </div>
        </div>
    </n-form>
    <downloadModal v-model:showModal="showModal" model-type="wd" />
</template>
