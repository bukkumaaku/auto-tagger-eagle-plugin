<script lang="ts" setup>
    import { NThing, NSpace, NIcon } from "naive-ui";
    import { CloseOutline, RemoveOutline } from "@vicons/ionicons5";
    import { t } from "../api/backen";

    const handleMinimize = async () => {
        await eagle.window.minimize();
    };
    const handleClose = async () => {
        await eagle.window.hide();
    };
</script>
<template>
    <n-space
        justify="space-between"
        style="
            width: 100%;
            height: 30px;
            position: fixed;
            top: 0px;
            left: 0px;
            -webkit-app-region: drag;
            z-index: 10000;
            border-bottom: 2px solid rgb(31, 34, 37);
        "
    >
        <n-thing
            style="
                height: 30px;
                line-height: 30px;
                margin-left: 10px;
                color: azure;
            "
            >{{ t("index.title") }}</n-thing
        >
        <div class="titlebar-actions">
            <button
                type="button"
                class="titlebar-button"
                :title="t('window.minimize')"
                :aria-label="t('window.minimize')"
                @click="handleMinimize"
            >
                <n-icon :size="18"><RemoveOutline /></n-icon>
            </button>
            <button
                type="button"
                class="titlebar-button titlebar-close"
                :title="t('window.close')"
                :aria-label="t('window.close')"
                @click="handleClose"
            >
                <n-icon :size="18"><CloseOutline /></n-icon>
            </button>
        </div>
    </n-space>
</template>

<style scoped>
    .titlebar-actions {
        height: 30px;
        display: flex;
        -webkit-app-region: no-drag;
    }

    .titlebar-button {
        width: 40px;
        height: 30px;
        display: inline-grid;
        place-items: center;
        border: 0;
        border-radius: 0;
        background: transparent;
        color: inherit;
        cursor: pointer;
        -webkit-app-region: no-drag;
    }

    .titlebar-button:hover {
        background: rgba(255, 255, 255, 0.1);
    }

    .titlebar-close:hover {
        background: #c42b1c;
        color: #fff;
    }
</style>
