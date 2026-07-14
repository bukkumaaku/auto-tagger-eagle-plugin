<script setup lang="ts">
    import {
        NConfigProvider,
        NNotificationProvider,
        NDialogProvider,
        darkTheme,
        NGlobalStyle,
    } from "naive-ui";
    import MainContent from "./mainContent.vue";
    import titleBar from "./components/titleBar.vue";
</script>

<template>
    <titleBar />
    <n-config-provider :theme="darkTheme">
        <n-global-style />
        <n-dialog-provider>
            <n-notification-provider placement="bottom-right">
                <MainContent id="mainContent" />
            </n-notification-provider>
        </n-dialog-provider>
    </n-config-provider>
</template>

<style scoped>
    #mainContent {
        height: 100%;
    }
    .n-config-provider {
        height: 100%;
    }
</style>
