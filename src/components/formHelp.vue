<script setup lang="ts">
    import { NTooltip } from "naive-ui";
    import questionMarkIcon from "../assets/images/img.icons8.png";

    withDefaults(
        defineProps<{
            content: string;
            html?: boolean;
        }>(),
        { html: false },
    );
</script>

<template>
    <n-tooltip trigger="hover">
        <template #trigger>
            <img
                :src="questionMarkIcon"
                class="form-help-icon"
                alt=""
                draggable="false"
            />
        </template>
        <div v-if="html" class="form-help-content" v-html="content"></div>
        <div v-else class="form-help-content">{{ content }}</div>
    </n-tooltip>
</template>

<style scoped>
    .form-help-icon {
        display: block;
        flex: none;
        width: 19px;
        height: 19px;
        margin-right: 5px;
        cursor: help;
        user-select: none;
    }

    .form-help-content {
        max-width: 360px;
        line-height: 1.55;
        white-space: normal;
    }
</style>
