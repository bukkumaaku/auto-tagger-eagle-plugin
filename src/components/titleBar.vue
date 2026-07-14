<script lang="ts" setup>
    import { NThing, NSpace, NButton } from "naive-ui";
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
        <n-thing>
            <n-button
                @click="handleMinimize"
                size="small"
                quaternary
                style="-webkit-app-region: no-drag"
            >
                <img
                    src="../assets/images/min.png"
                    alt="min"
                    style="width: 18px; height: 18px"
                />
            </n-button>
            <n-button
                @click="handleClose"
                size="small"
                quaternary
                style="-webkit-app-region: no-drag"
            >
                <img
                    src="../assets/images/close.png"
                    style="width: 18px; height: 18px"
                    alt="close"
                />
            </n-button>
        </n-thing>
    </n-space>
</template>
