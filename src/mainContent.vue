<script setup lang="ts">
    import wdTypeTagger from "./components/wdTypeTagger.vue";
    import llmTypeTagger from "./components/llmTypeTagger.vue";
    import semanticSearch from "./components/semanticSearch.vue";
    import settingsPanel from "./components/settingsPanel.vue";
    import taskCenter from "./components/taskCenter.vue";
    import {
        type MenuOption,
        NLayoutSider,
        NMenu,
        NLayout,
        NIcon,
    } from "naive-ui";
    import {
        PricetagsOutline,
        MenuOutline,
        SettingsOutline,
        SparklesOutline,
        ImagesOutline,
    } from "@vicons/ionicons5";
    import { h, ref, type Component } from "vue";
    import { backenAPI, t, dialog, config, notification } from "./api/backen";

    const collapsed = ref(true);
    const currentPage = ref("wdtype");
    let updateCheckStarted = false;

    async function openReleasePage(url: string) {
        if (eagle.shell?.openExternal) {
            await eagle.shell.openExternal(url);
            return;
        }

        const electron = window.require?.("electron");
        if (electron?.shell?.openExternal) {
            await electron.shell.openExternal(url);
            return;
        }

        const opened = window.open(url, "_blank");
        if (!opened) {
            throw new Error("无法打开 Release 页面");
        }
    }

    async function checkForUpdate() {
        if (updateCheckStarted) return;
        updateCheckStarted = true;

        try {
            const result = await backenAPI.checkForUpdate();
            if (!result.updateAvailable) return;

            dialog.warning({
                title: t.value("update.title"),
                content:
                    `${t.value("update.current_version")}${result.currentVersion}` +
                    `${t.value("update.latest_version")}${result.latestVersion}` +
                    t.value("update.open_prompt"),
                positiveText: t.value("update.open_release"),
                negativeText: t.value("update.later"),
                onPositiveClick: async () => {
                    try {
                        await openReleasePage(result.releaseUrl);
                    } catch (error) {
                        notification(
                            `${t.value("update.open_failed")}: ${error instanceof Error ? error.message : String(error)}`,
                            "error",
                        );
                    }
                },
            });
        } catch (error) {
            updateCheckStarted = false;
            console.warn("检查更新失败", error);
        }
    }

    const renderIcon = (icon: Component) => {
        return () => h(NIcon, null, { default: () => h(icon) });
    };

    const menuOptions: MenuOption[] = [
        {
            label: "WD 模型打标签",
            key: "wdtype",
            icon: renderIcon(PricetagsOutline),
        },
        {
            label: "LLM 图像理解",
            key: "llmtype",
            icon: renderIcon(SparklesOutline),
        },
        {
            label: "语义搜索",
            key: "semantic-search",
            icon: renderIcon(ImagesOutline),
        },
        {
            label: "设置",
            key: "settings",
            icon: renderIcon(SettingsOutline),
        },
    ];

    const clickOption = (key: string) => {
        if (backenAPI.is_processing && key !== currentPage.value) {
            notification(
                t.value("model_download_window.process_checking"),
                "warning",
            );
            return;
        }
        currentPage.value = key;
    };

    window.eagle.onPluginRun(async (plugin: any) => {
        window.pluginPath = plugin.path;
        t.value = (key: any) => i18next.t(key);
        await backenAPI.getConfig();
        if (config.modelLocation === "") {
            dialog.error({
                title: "错误",
                content:
                    "未设置模型文件路径。点击确认后请选择一个位置安置模型文件。",
                positiveText: "OK",
                maskClosable: false,
                closeOnEsc: false,
                closable: false,
                onPositiveClick: async () => {
                    const result = await eagle.dialog.showOpenDialog({
                        properties: ["openDirectory"],
                    });
                    if (!result.canceled && result.filePaths?.[0]) {
                        config.modelLocation = result.filePaths[0];
                        await backenAPI.setConfig();
                        void checkForUpdate();
                    }
                },
            });
        } else {
            void checkForUpdate();
        }
    });
</script>

<template style="height: 100%">
    <n-layout has-sider style="height: 100%; padding-top: 30px">
        <n-layout has-sider style="height: 100%">
            <n-layout-sider
                bordered
                collapse-mode="width"
                :collapsed-width="64"
                :width="240"
                :collapsed="collapsed"
                @collapse="collapsed = true"
                @expand="collapsed = false"
            >
                <button
                    type="button"
                    class="sider-toggle"
                    :class="{ 'sider-toggle--collapsed': collapsed }"
                    :title="collapsed ? '展开' : '收起'"
                    @click="collapsed = !collapsed"
                >
                    <n-icon
                        :size="22"
                        :style="{
                            transform: collapsed
                                ? 'rotate(0deg)'
                                : 'rotate(90deg)',
                            transition:
                                'transform 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
                        }"
                    >
                        <MenuOutline />
                    </n-icon>
                </button>
                <n-menu
                    :value="currentPage"
                    :collapsed="collapsed"
                    :collapsed-width="64"
                    :collapsed-icon-size="22"
                    :options="menuOptions"
                    @update:value="clickOption"
                />
            </n-layout-sider>
            <n-layout>
                <wdTypeTagger v-if="currentPage == 'wdtype'" />
                <llmTypeTagger v-if="currentPage == 'llmtype'" />
                <semanticSearch v-if="currentPage == 'semantic-search'" />
                <settingsPanel v-if="currentPage == 'settings'" />
            </n-layout>
        </n-layout>
    </n-layout>
    <taskCenter />
</template>

<style scoped>
    .sider-toggle {
        width: 100%;
        height: 42px;
        display: flex;
        align-items: center;
        justify-content: flex-start;
        padding: 0 0 0 32px !important;
        border: 0;
        border-radius: 0;
        background: transparent !important;
        color: inherit;
        cursor: pointer;
        transition: padding-left 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    }

    .sider-toggle--collapsed {
        padding-left: 21px !important;
    }
</style>
