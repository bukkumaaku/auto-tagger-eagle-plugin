<script setup lang="ts">
import { computed } from "vue";
import { NButton, NIcon, NSpace } from "naive-ui";
import { CloseOutline, PauseOutline, PlayOutline } from "@vicons/ionicons5";
import { pauseTask, requestTaskCancel, resumeTask, taskHistory } from "../services/taskCenter";

const props = defineProps<{ taskId: string }>();
const task = computed(() => taskHistory.value.find((item) => item.id === props.taskId));
</script>

<template>
    <n-space v-if="task && (task.status === 'running' || task.status === 'paused')" size="small">
        <n-button v-if="task.status === 'running'" secondary @click="pauseTask(task.id)">
            <template #icon><n-icon><PauseOutline /></n-icon></template>
            暂停
        </n-button>
        <n-button v-else secondary type="warning" @click="resumeTask(task.id)">
            <template #icon><n-icon><PlayOutline /></n-icon></template>
            继续
        </n-button>
        <n-button secondary type="error" @click="requestTaskCancel(task.id)">
            <template #icon><n-icon><CloseOutline /></n-icon></template>
            取消
        </n-button>
    </n-space>
</template>
