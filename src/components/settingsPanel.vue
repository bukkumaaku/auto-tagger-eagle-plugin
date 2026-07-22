<script setup lang="ts">
    import {
        NButton,
        NForm,
        NFormItem,
        NIcon,
        NInput,
        NInputNumber,
        NSelect,
        NSlider,
        NDivider,
    } from "naive-ui";
    import { CloudDownloadOutline } from "@vicons/ionicons5";
    import { onBeforeUnmount, onMounted, ref, watch, type Ref } from "vue";
    import { backenAPI, config, notification, t } from "../api/backen";
    import { DEFAULT_GEMINI_EMBEDDING_MODEL } from "../constants/embedding";
    import downloadModal from "./downloadModal.vue";
    import FormHelp from "./formHelp.vue";
    import type { RemoteEmbeddingProfile, RemoteLlmProfile } from "../protocol";

    type SettingsFormData = {
        modelLocation: string;
        llmRunnerPath: string;
        endpoint: string;
        apiKey: string;
        embeddingModelName: string;
        embeddingProvider: "open_ai" | "gemini";
        embeddingDimension: number;
        negativePromptWeight: number;
        embeddingRemoteProfiles: RemoteEmbeddingProfile[];
        embeddingRemoteProfileId: string;
        llmRemoteProfiles: RemoteLlmProfile[];
    };

    const defaultFormData: SettingsFormData = {
        modelLocation: "",
        llmRunnerPath: "",
        endpoint: "",
        apiKey: "",
        embeddingModelName: "",
        embeddingProvider: "open_ai",
        embeddingDimension: 1536,
        negativePromptWeight: 0.3,
        embeddingRemoteProfiles: [],
        embeddingRemoteProfileId: "",
        llmRemoteProfiles: [],
    };

    const cloneFormData = (value: SettingsFormData): SettingsFormData => ({
        ...value,
        embeddingRemoteProfiles: value.embeddingRemoteProfiles.map((profile) => ({ ...profile })),
        llmRemoteProfiles: value.llmRemoteProfiles.map((profile) => ({ ...profile })),
    });

    let originalConfig: SettingsFormData = cloneFormData(defaultFormData);
    const formData: Ref<SettingsFormData> = ref(cloneFormData(defaultFormData));
    const isReady = ref(false);
    const selectedLlmProfileId = ref("");
    const showLlamafileDownload = ref(false);
    let saveTimer: ReturnType<typeof setTimeout> | undefined;
    let suspendAutoSave = false;

    const normalizeNegativePromptWeight = (value: unknown) => {
        const numeric = Number(value);
        return Number.isFinite(numeric)
            ? Math.min(1, Math.max(0, numeric))
            : 0.3;
    };

    const syncConfigToForm = () => {
        const nextFormData = {
            ...defaultFormData,
            ...Object.fromEntries(
                Object.keys(defaultFormData).map((key) => [
                    key,
                    config[key] ??
                        defaultFormData[key as keyof SettingsFormData],
                ]),
            ),
        } as SettingsFormData;
        nextFormData.negativePromptWeight = normalizeNegativePromptWeight(
            nextFormData.negativePromptWeight,
        );
        formData.value = cloneFormData(nextFormData);
        if (!formData.value.embeddingRemoteProfileId) formData.value.embeddingRemoteProfileId = formData.value.embeddingRemoteProfiles[0]?.id || "";
        if (formData.value.embeddingRemoteProfiles.length === 0 && formData.value.endpoint && formData.value.embeddingModelName) addRemoteProfile(false);
        loadActiveProfile();
        if (
            formData.value.llmRemoteProfiles.length === 0 &&
            config.llmEndpoint &&
            config.llmRemoteModel
        ) {
            formData.value.llmRemoteProfiles.push({
                id: "llm-legacy",
                name: t.value("settings.remote_llm_legacy"),
                provider: config.llmProvider === "gemini" ? "gemini" : "open_ai",
                endpoint: config.llmEndpoint,
                apiKey: config.llmApiKey,
                model: config.llmRemoteModel,
            });
        }
        selectedLlmProfileId.value = formData.value.llmRemoteProfiles.some(
            (profile) => profile.id === config.llmRemoteProfileId,
        )
            ? config.llmRemoteProfileId
            : formData.value.llmRemoteProfiles[0]?.id || "";
    };

    const activeProfile = () =>
        formData.value.embeddingRemoteProfiles.find(
            (profile) =>
                profile.id === formData.value.embeddingRemoteProfileId,
        );
    const loadActiveProfile = () => {
        const profile = activeProfile();
        if (!profile) return;
        formData.value.embeddingProvider = profile.provider;
        formData.value.endpoint = profile.endpoint;
        formData.value.apiKey = profile.apiKey;
        formData.value.embeddingModelName = profile.model;
        formData.value.embeddingDimension = profile.dimension || 1536;
    };
    const addRemoteProfile = (select = true) => {
        const id = `embedding-${Date.now()}-${Math.random().toString(16).slice(2)}`;
        formData.value.embeddingRemoteProfiles.push({
            id,
            name: `远程接口 ${formData.value.embeddingRemoteProfiles.length + 1}`,
            provider: formData.value.embeddingProvider,
            endpoint: formData.value.endpoint,
            apiKey: formData.value.apiKey,
            model: formData.value.embeddingModelName,
            dimension: formData.value.embeddingDimension,
        });
        formData.value.embeddingRemoteProfileId = id;
        if (select) loadActiveProfile();
    };
    const removeRemoteProfile = () => {
        const id = formData.value.embeddingRemoteProfileId;
        formData.value.embeddingRemoteProfiles =
            formData.value.embeddingRemoteProfiles.filter(
                (profile) => profile.id !== id,
            );
        formData.value.embeddingRemoteProfileId =
            formData.value.embeddingRemoteProfiles[0]?.id || "";
        loadActiveProfile();
    };
    const updateActiveProfile = () => {
        const profile = activeProfile();
        if (!profile) return;
        Object.assign(profile, {
            provider: formData.value.embeddingProvider,
            endpoint: formData.value.endpoint,
            apiKey: formData.value.apiKey,
            model: formData.value.embeddingModelName,
            dimension: formData.value.embeddingDimension,
        });
    };
    const activeLlmProfile = () => formData.value.llmRemoteProfiles.find(
        (profile) => profile.id === selectedLlmProfileId.value,
    );
    const addLlmProfile = () => {
        const id = `llm-${Date.now()}-${Math.random().toString(16).slice(2)}`;
        formData.value.llmRemoteProfiles.push({
            id,
            name: `${t.value("settings.remote_llm_default_name")} ${formData.value.llmRemoteProfiles.length + 1}`,
            provider: "open_ai",
            endpoint: "",
            apiKey: "",
            model: "",
        });
        selectedLlmProfileId.value = id;
    };
    const removeLlmProfile = () => {
        const removedId = selectedLlmProfileId.value;
        formData.value.llmRemoteProfiles = formData.value.llmRemoteProfiles.filter(
            (profile) => profile.id !== removedId,
        );
        selectedLlmProfileId.value = formData.value.llmRemoteProfiles[0]?.id || "";
        if (config.llmRemoteProfileId === removedId) {
            config.llmRemoteProfileId = selectedLlmProfileId.value;
            if (!selectedLlmProfileId.value) config.llmProvider = "local";
        }
    };

    const syncFormToConfig = async () => {
        updateActiveProfile();
        formData.value.negativePromptWeight = normalizeNegativePromptWeight(
            formData.value.negativePromptWeight,
        );
        Object.entries(cloneFormData(formData.value)).forEach(([key, value]) => {
            config[key] = value;
        });
        if (
            config.llmProvider !== "local" &&
            !formData.value.llmRemoteProfiles.some(
                (profile) => profile.id === config.llmRemoteProfileId,
            )
        ) {
            const fallback = formData.value.llmRemoteProfiles[0];
            config.llmRemoteProfileId = fallback?.id || "";
            config.llmProvider = fallback?.provider || "local";
        }
        await backenAPI.setConfig();
    };

    onMounted(async () => {
        await backenAPI.getConfig();
        syncConfigToForm();
        originalConfig = cloneFormData(formData.value);
        isReady.value = true;
    });

    onBeforeUnmount(() => {
        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = undefined;
        if (isReady.value) {
            void syncFormToConfig().catch((error) =>
                console.error("保存设置失败", error),
            );
        }
    });

    watch(
        formData,
        () => {
            if (!isReady.value || suspendAutoSave) return;
            if (saveTimer) clearTimeout(saveTimer);
            saveTimer = setTimeout(() => {
                saveTimer = undefined;
                void syncFormToConfig().catch((error) =>
                    console.error("保存设置失败", error),
                );
            }, 200);
        },
        { deep: true },
    );
    watch(() => formData.value.embeddingRemoteProfileId, loadActiveProfile);

    watch(
        () => formData.value.embeddingProvider,
        (provider) => {
            if (
                provider === "gemini" &&
                !formData.value.embeddingModelName.trim()
            ) {
                formData.value.embeddingModelName =
                    DEFAULT_GEMINI_EMBEDDING_MODEL;
            }
        },
    );

    const chooseDirectory = async (key: "modelLocation") => {
        const result = await eagle.dialog.showOpenDialog({
            properties: ["openDirectory"],
        });
        if (!result.canceled && result.filePaths?.[0]) {
            formData.value[key] = result.filePaths[0];
        }
    };

    const chooseFile = async (key: "llmRunnerPath") => {
        const result = await eagle.dialog.showOpenDialog({
            properties: ["openFile"],
        });
        if (!result.canceled && result.filePaths?.[0]) {
            formData.value[key] = result.filePaths[0];
        }
    };

    const openLlamafileDownload = async () => {
        if (!formData.value.modelLocation) {
            notification(
                t.value("model_download_window.select_model_location_notice"),
                "warning",
            );
            return;
        }
        await syncFormToConfig();
        showLlamafileDownload.value = true;
    };

    const handleLlamafileDownloaded = async (paths: string[]) => {
        if (!paths[0]) return;
        formData.value.llmRunnerPath = paths[0];
        await syncFormToConfig();
        notification(t.value("settings.llamafile_downloaded"));
    };

    const save = async () => {
        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = undefined;
        await syncFormToConfig();
        notification(t.value("settings.saved"));
    };

    const reset = async () => {
        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = undefined;
        suspendAutoSave = true;
        try {
            formData.value = cloneFormData(originalConfig);
            if (!formData.value.llmRemoteProfiles.some(
                (profile) => profile.id === selectedLlmProfileId.value,
            )) {
                selectedLlmProfileId.value =
                    formData.value.llmRemoteProfiles[0]?.id || "";
            }
            await syncFormToConfig();
        } finally {
            suspendAutoSave = false;
        }
    };
</script>

<template>
    <n-form
        :model="formData"
        label-placement="left"
        :label-width="150"
        class="settings-form"
    >
        <n-form-item :label="t('settings.model_location')" path="modelLocation">
            <FormHelp :content="t('settings.model_location_desc')" />
            <n-input v-model:value="formData.modelLocation" />
            <n-button @click="chooseDirectory('modelLocation')">
                {{ t("settings.choose") }}
            </n-button>
        </n-form-item>

        <n-form-item :label="t('settings.llm_runner')" path="llmRunnerPath">
            <FormHelp :content="t('settings.llm_runner_desc')" />
            <n-input
                v-model:value="formData.llmRunnerPath"
                :placeholder="t('settings.llm_runner_placeholder')"
            />
            <n-button @click="chooseFile('llmRunnerPath')">
                {{ t("settings.choose") }}
            </n-button>
            <n-button
                type="primary"
                secondary
                :disabled="backenAPI.is_processing"
                @click="openLlamafileDownload"
            >
                <template #icon>
                    <n-icon><CloudDownloadOutline /></n-icon>
                </template>
                {{ t("settings.download_llamafile") }}
            </n-button>
        </n-form-item>

        <n-divider title-placement="left">
            {{ t("settings.remote_llm") }}
        </n-divider>

        <n-form-item :label="t('settings.remote_llm_profiles')">
            <n-select
                v-model:value="selectedLlmProfileId"
                :options="formData.llmRemoteProfiles.map((profile) => ({
                    label: profile.name || profile.model || profile.endpoint,
                    value: profile.id,
                }))"
                :placeholder="t('settings.remote_llm_select')"
            />
            <n-button @click="addLlmProfile">
                {{ t("settings.remote_llm_add") }}
            </n-button>
            <n-button
                :disabled="!selectedLlmProfileId"
                @click="removeLlmProfile"
            >
                {{ t("settings.remote_llm_remove") }}
            </n-button>
        </n-form-item>

        <template v-if="activeLlmProfile()">
            <n-form-item :label="t('settings.remote_llm_name')">
                <n-input
                    v-model:value="activeLlmProfile()!.name"
                    :placeholder="t('settings.remote_llm_name_placeholder')"
                />
            </n-form-item>
            <n-form-item :label="t('settings.remote_llm_provider')">
                <n-select
                    v-model:value="activeLlmProfile()!.provider"
                    :options="[
                        { label: 'OpenAI Compatible', value: 'open_ai' },
                        { label: 'Gemini REST', value: 'gemini' },
                    ]"
                />
            </n-form-item>
            <n-form-item :label="t('settings.remote_llm_endpoint')">
                <n-input
                    v-model:value="activeLlmProfile()!.endpoint"
                    :placeholder="activeLlmProfile()!.provider === 'gemini'
                        ? 'https://generativelanguage.googleapis.com'
                        : 'https://api.openai.com'"
                />
            </n-form-item>
            <n-form-item :label="t('settings.remote_llm_api_key')">
                <n-input
                    v-model:value="activeLlmProfile()!.apiKey"
                    type="password"
                    show-password-on="click"
                />
            </n-form-item>
            <n-form-item :label="t('settings.remote_llm_model')">
                <n-input
                    v-model:value="activeLlmProfile()!.model"
                    :placeholder="activeLlmProfile()!.provider === 'gemini'
                        ? 'gemini-2.5-flash'
                        : 'gpt-4.1-mini'"
                />
            </n-form-item>
        </template>

        <n-divider title-placement="left">
            {{ t("settings.remote_embedding") }}
        </n-divider>

        <n-form-item
            :label="t('settings.embedding_provider')"
            path="embeddingProvider"
        >
            <n-select
                v-model:value="formData.embeddingRemoteProfileId"
                :options="formData.embeddingRemoteProfiles.map((profile) => ({
                    label: profile.name || profile.model || profile.endpoint,
                    value: profile.id,
                }))"
                placeholder="选择远程接口"
            />
            <n-button @click="addRemoteProfile()">新增</n-button>
            <n-button :disabled="!formData.embeddingRemoteProfileId" @click="removeRemoteProfile">删除</n-button>
        </n-form-item>

        <n-form-item label="接口名称">
            <n-input
                v-if="activeProfile()"
                v-model:value="activeProfile()!.name"
                placeholder="例如：主站、备用中转站"
            />
        </n-form-item>

        <n-form-item
            :label="t('settings.embedding_provider')"
            path="embeddingProvider"
        >
            <FormHelp :content="t('settings.embedding_provider_desc')" />
            <n-select
                v-model:value="formData.embeddingProvider"
                :options="[
                    {
                        label: t('settings.embedding_provider_openai'),
                        value: 'open_ai',
                    },
                    {
                        label: t('settings.embedding_provider_gemini'),
                        value: 'gemini',
                    },
                ]"
            />
        </n-form-item>

        <n-form-item
            :label="t('settings.embedding_endpoint')"
            path="endpoint"
        >
            <FormHelp :content="t('settings.embedding_endpoint_desc')" />
            <n-input
                v-model:value="formData.endpoint"
                :placeholder="
                    formData.embeddingProvider === 'gemini'
                        ? 'https://generativelanguage.googleapis.com'
                        : 'https://api.openai.com'
                "
            />
        </n-form-item>

        <n-form-item :label="t('settings.embedding_api_key')" path="apiKey">
            <FormHelp :content="t('settings.embedding_api_key_desc')" />
            <n-input
                v-model:value="formData.apiKey"
                type="password"
                show-password-on="click"
            />
        </n-form-item>

        <n-form-item
            :label="t('settings.embedding_model_name')"
            path="embeddingModelName"
        >
            <FormHelp :content="t('settings.embedding_model_name_desc')" />
            <n-input
                v-model:value="formData.embeddingModelName"
                :placeholder="
                    formData.embeddingProvider === 'gemini'
                        ? DEFAULT_GEMINI_EMBEDDING_MODEL
                        : t('settings.embedding_model_name_placeholder')
                "
            />
        </n-form-item>

        <n-form-item
            v-if="formData.embeddingProvider === 'gemini'"
            :label="t('settings.embedding_dimension')"
            path="embeddingDimension"
        >
            <FormHelp :content="t('settings.embedding_dimension_desc')" />
            <n-input-number
                v-model:value="formData.embeddingDimension"
                :min="128"
                :max="3072"
                :step="1"
            />
        </n-form-item>

        <n-divider title-placement="left">
            {{ t("settings.semantic_search") }}
        </n-divider>

        <n-form-item
            :label="t('settings.negative_prompt_weight')"
            path="negativePromptWeight"
        >
            <FormHelp :content="t('settings.negative_prompt_weight_desc')" />
            <div class="weight-control">
                <n-slider
                    v-model:value="formData.negativePromptWeight"
                    :min="0"
                    :max="1"
                    :step="0.05"
                />
                <n-input-number
                    v-model:value="formData.negativePromptWeight"
                    :min="0"
                    :max="1"
                    :step="0.05"
                    :precision="2"
                />
            </div>
        </n-form-item>

        <div class="action-row">
            <n-button type="primary" @click="save">
                {{ t("settings.save") }}
            </n-button>
            <n-button @click="reset">{{ t("index.reset") }}</n-button>
        </div>
    </n-form>
    <downloadModal
        v-model:showModal="showLlamafileDownload"
        model-type="llm"
        runner-only
        :reload-on-complete="false"
        @completed="handleLlamafileDownloaded"
    />
</template>

<style scoped>
    .settings-form {
        width: min(900px, calc(100% - 48px));
        margin: 30px auto 0;
        padding-bottom: 40px;
    }

    .settings-form :deep(.n-form-item-blank) {
        min-width: 0;
        gap: 10px;
    }

    .settings-form :deep(.n-input),
    .settings-form :deep(.n-select),
    .settings-form :deep(.n-input-number) {
        min-width: 0;
        flex: 1 1 0;
    }

    .settings-form :deep(.n-button) {
        flex: 0 0 auto;
    }

    .weight-control {
        display: grid;
        grid-template-columns: minmax(180px, 1fr) 120px;
        align-items: center;
        gap: 16px;
        width: 100%;
    }

    .action-row {
        display: flex;
        gap: 10px;
        justify-content: flex-end;
    }

    @media (max-width: 720px) {
        .settings-form {
            width: calc(100% - 24px);
            margin-top: 18px;
        }

        .settings-form :deep(.n-form-item-blank) {
            flex-wrap: wrap;
        }

        .settings-form :deep(.n-form-item-blank > .n-input),
        .settings-form :deep(.n-form-item-blank > .n-select) {
            flex-basis: calc(100% - 30px);
        }

        .weight-control {
            grid-template-columns: 1fr;
        }
    }
</style>
