<script setup lang="ts">
    import {
        NButton,
        NForm,
        NFormItem,
        NIcon,
        NInput,
        NInputNumber,
        NSelect,
    } from "naive-ui";
    import { CloudDownloadOutline } from "@vicons/ionicons5";
    import { onBeforeUnmount, onMounted, ref, watch, type Ref } from "vue";
    import { backenAPI, config, notification, t } from "../api/backen";
    import { DEFAULT_GEMINI_EMBEDDING_MODEL } from "../constants/embedding";
    import downloadModal from "./downloadModal.vue";
    import FormHelp from "./formHelp.vue";
    import type { RemoteEmbeddingProfile } from "../protocol";

    type SettingsFormData = {
        modelLocation: string;
        llmRunnerPath: string;
        endpoint: string;
        apiKey: string;
        embeddingModelName: string;
        embeddingProvider: "open_ai" | "gemini";
        embeddingDimension: number;
        embeddingRemoteProfiles: RemoteEmbeddingProfile[];
        embeddingRemoteProfileId: string;
    };

    const defaultFormData: SettingsFormData = {
        modelLocation: "",
        llmRunnerPath: "",
        endpoint: "",
        apiKey: "",
        embeddingModelName: "",
        embeddingProvider: "open_ai",
        embeddingDimension: 1536,
        embeddingRemoteProfiles: [],
        embeddingRemoteProfileId: "",
    };

    const cloneFormData = (value: SettingsFormData): SettingsFormData => ({ ...value, embeddingRemoteProfiles: value.embeddingRemoteProfiles.map((profile) => ({ ...profile })) });

    let originalConfig: SettingsFormData = cloneFormData(defaultFormData);
    const formData: Ref<SettingsFormData> = ref(cloneFormData(defaultFormData));
    const isReady = ref(false);
    const showLlamafileDownload = ref(false);
    let saveTimer: ReturnType<typeof setTimeout> | undefined;

    const syncConfigToForm = () => {
        formData.value = {
            ...defaultFormData,
            ...Object.fromEntries(
                Object.keys(defaultFormData).map((key) => [
                    key,
                    config[key] ??
                        defaultFormData[key as keyof SettingsFormData],
                ]),
            ),
        } as SettingsFormData;
        if (!formData.value.embeddingRemoteProfileId) formData.value.embeddingRemoteProfileId = formData.value.embeddingRemoteProfiles[0]?.id || "";
        if (formData.value.embeddingRemoteProfiles.length === 0 && formData.value.endpoint && formData.value.embeddingModelName) addRemoteProfile(false);
        loadActiveProfile();
    };

    const activeProfile = () => formData.value.embeddingRemoteProfiles.find((profile) => profile.id === formData.value.embeddingRemoteProfileId);
    const loadActiveProfile = () => { const profile = activeProfile(); if (!profile) return; formData.value.embeddingProvider = profile.provider; formData.value.endpoint = profile.endpoint; formData.value.apiKey = profile.apiKey; formData.value.embeddingModelName = profile.model; formData.value.embeddingDimension = profile.dimension || 1536; };
    const addRemoteProfile = (select = true) => { const id = `embedding-${Date.now()}-${Math.random().toString(16).slice(2)}`; formData.value.embeddingRemoteProfiles.push({ id, name: `远程接口 ${formData.value.embeddingRemoteProfiles.length + 1}`, provider: formData.value.embeddingProvider, endpoint: formData.value.endpoint, apiKey: formData.value.apiKey, model: formData.value.embeddingModelName, dimension: formData.value.embeddingDimension }); formData.value.embeddingRemoteProfileId = id; if (select) loadActiveProfile(); };
    const removeRemoteProfile = () => { const id = formData.value.embeddingRemoteProfileId; formData.value.embeddingRemoteProfiles = formData.value.embeddingRemoteProfiles.filter((profile) => profile.id !== id); formData.value.embeddingRemoteProfileId = formData.value.embeddingRemoteProfiles[0]?.id || ""; loadActiveProfile(); };
    const updateActiveProfile = () => { const profile = activeProfile(); if (!profile) return; Object.assign(profile, { provider: formData.value.embeddingProvider, endpoint: formData.value.endpoint, apiKey: formData.value.apiKey, model: formData.value.embeddingModelName, dimension: formData.value.embeddingDimension }); };

    const syncFormToConfig = async () => {
        updateActiveProfile();
        Object.entries(formData.value).forEach(([key, value]) => {
            config[key] = value;
        });
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
            if (!isReady.value) return;
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

    const chooseDirectory = async (key: keyof SettingsFormData) => {
        const result = await eagle.dialog.showOpenDialog({
            properties: ["openDirectory"],
        });
        if (!result.canceled && result.filePaths?.[0]) {
            formData.value[key] = result.filePaths[0] as never;
        }
    };

    const chooseFile = async (key: keyof SettingsFormData) => {
        const result = await eagle.dialog.showOpenDialog({
            properties: ["openFile"],
        });
        if (!result.canceled && result.filePaths?.[0]) {
            formData.value[key] = result.filePaths[0] as never;
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
        formData.value = cloneFormData(originalConfig);
        await syncFormToConfig();
    };
</script>

<template>
    <n-form
        :model="formData"
        label-placement="left"
        label-width="auto"
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
                style="margin: auto; margin-right: 0px"
                @click="openLlamafileDownload"
            >
                <template #icon>
                    <n-icon><CloudDownloadOutline /></n-icon>
                </template>
                {{ t("settings.download_llamafile") }}
            </n-button>
        </n-form-item>

        <n-form-item
            :label="t('settings.embedding_provider')"
            path="embeddingProvider"
        >
            <n-select v-model:value="formData.embeddingRemoteProfileId" :options="formData.embeddingRemoteProfiles.map((profile) => ({ label: profile.name || profile.model || profile.endpoint, value: profile.id }))" placeholder="选择远程接口" />
            <n-button @click="addRemoteProfile()">新增</n-button>
            <n-button :disabled="!formData.embeddingRemoteProfileId" @click="removeRemoteProfile">删除</n-button>
        </n-form-item>

        <n-form-item label="接口名称">
            <n-input v-if="activeProfile()" v-model:value="activeProfile()!.name" placeholder="例如：主站、备用中转站" />
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
        width: 75%;
        margin: 30px auto 0;
    }

    .settings-form :deep(.n-input) {
        margin-right: 12px;
    }

    .action-row {
        display: flex;
        gap: 10px;
        justify-content: flex-end;
    }
</style>
