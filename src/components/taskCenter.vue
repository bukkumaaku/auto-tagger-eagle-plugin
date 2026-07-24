<script setup lang="ts">
    import {
        NBadge,
        NButton,
        NDrawer,
        NDrawerContent,
        NEmpty,
        NIcon,
        NProgress,
        NTabPane,
        NTabs,
        NTag,
    } from "naive-ui";
    import { CloseOutline, EyeOutline, ListOutline, PauseOutline, PlayOutline, TrashOutline } from "@vicons/ionicons5";
    import { computed, ref } from "vue";
    import {
        activeTask,
        clearFailure,
        clearFailures,
        clearFinishedTasks,
        failureRecords,
        pauseTask,
        requestTaskCancel,
        resumeTask,
        taskHistory,
        type TaskRecord,
    } from "../services/taskCenter";

    const visible = ref(false);
    const percentage = (task: TaskRecord) =>
        task.total > 0
            ? Number(Math.min(100, (task.completed / task.total) * 100).toFixed(2))
            : 0;
    const statusType = (task: TaskRecord) =>
        task.status === "failed"
            ? "error"
            : task.status === "completed"
              ? "success"
              : task.status === "cancelled"
                ? "default"
              : task.status === "paused"
                ? "warning"
                : "info";
    const statusText = (task: TaskRecord) =>
        ({
            running: "运行中",
            paused: "已暂停",
            completed: "已完成",
            failed: "失败",
            cancelled: "已取消",
        })[task.status];
    const kindText = (kind: TaskRecord["kind"]) =>
        ({
            wd: "WD 打标",
            llm: "LLM",
            embedding: "向量索引",
            search: "语义搜索",
            download: "下载",
        })[kind];
    const activePercentage = computed(() =>
        activeTask.value ? percentage(activeTask.value) : 0,
    );
    const openFailureItem = async (itemId?: string) => {
        if (itemId) await eagle.item.open(itemId);
    };
</script>

<template>
    <div class="task-entry">
        <n-badge
            :value="failureRecords.length || undefined"
            :dot="failureRecords.length === 0 && Boolean(activeTask)"
            processing
        >
            <n-button circle secondary title="任务中心" @click="visible = true">
                <template #icon><n-icon><ListOutline /></n-icon></template>
            </n-button>
        </n-badge>
        <button
            v-if="activeTask"
            type="button"
            class="active-summary"
            @click="visible = true"
        >
            <span>{{ activeTask.title }}</span>
            <span>{{ activeTask.detail }}</span>
            <strong v-if="activeTask.total > 0">{{ activePercentage.toFixed(2) }}%</strong>
        </button>
    </div>

    <n-drawer
        v-model:show="visible"
        :width="420"
        placement="right"
        :drawer-style="{ top: '30px', height: 'calc(100% - 30px)' }"
    >
        <n-drawer-content title="任务中心" closable>
            <n-tabs type="line" animated>
                <n-tab-pane name="tasks" tab="任务">
                    <div class="task-toolbar">
                        <n-button
                            quaternary
                            :disabled="taskHistory.every((task) => task.status === 'running' || task.status === 'paused')"
                            @click="clearFinishedTasks"
                        >
                            <template #icon><n-icon><TrashOutline /></n-icon></template>
                            清理已完成
                        </n-button>
                    </div>
                    <n-empty v-if="taskHistory.length === 0" description="暂无任务" />
                    <div v-else class="task-list">
                        <article v-for="task in taskHistory" :key="task.id" class="task-item">
                    <div class="task-header">
                        <strong>{{ task.title }}</strong>
                        <n-tag size="small" :type="statusType(task)">
                            {{ statusText(task) }}
                        </n-tag>
                    </div>
                    <div class="task-detail">{{ task.detail }}</div>
                    <n-progress
                        v-if="task.total > 0"
                        type="line"
                        :percentage="percentage(task)"
                        :processing="task.status === 'running'"
                        :status="task.status === 'failed' ? 'error' : task.status === 'completed' ? 'success' : 'default'"
                        :format="(value: number) => `${value.toFixed(2)}%`"
                    />
                    <div v-else-if="task.status === 'running'" class="indeterminate">
                        <n-progress type="line" processing :percentage="0" :show-indicator="false" />
                    </div>
                    <div v-if="task.total > 0" class="task-count">
                        {{ task.completed }}/{{ task.total }}
                    </div>
                    <div v-if="task.controllable && (task.status === 'running' || task.status === 'paused')" class="task-actions">
                        <n-button v-if="task.status === 'running'" size="small" secondary @click="pauseTask(task.id)"><template #icon><n-icon><PauseOutline /></n-icon></template>暂停</n-button>
                        <n-button v-else size="small" secondary type="warning" @click="resumeTask(task.id)"><template #icon><n-icon><PlayOutline /></n-icon></template>继续</n-button>
                        <n-button size="small" secondary type="error" @click="requestTaskCancel(task.id)"><template #icon><n-icon><CloseOutline /></n-icon></template>取消</n-button>
                    </div>
                    <div v-if="task.error" class="task-error">{{ task.error }}</div>
                        </article>
                    </div>
                </n-tab-pane>
                <n-tab-pane name="failures" :tab="`失败项目 ${failureRecords.length}`">
                    <div class="task-toolbar">
                        <n-button quaternary :disabled="failureRecords.length === 0" @click="clearFailures">
                            <template #icon><n-icon><TrashOutline /></n-icon></template>
                            清空失败记录
                        </n-button>
                    </div>
                    <n-empty v-if="failureRecords.length === 0" description="暂无失败项目" />
                    <div v-else class="task-list">
                        <article v-for="failure in failureRecords" :key="failure.id" class="task-item">
                            <div class="task-header">
                                <strong>{{ failure.name || failure.path || "未知项目" }}</strong>
                                <n-tag size="small" type="error">{{ kindText(failure.kind) }}</n-tag>
                            </div>
                            <div v-if="failure.path" class="failure-path">{{ failure.path }}</div>
                            <div class="task-error">{{ failure.error }}</div>
                            <div class="failure-actions">
                                <n-button
                                    v-if="failure.itemId"
                                    size="small"
                                    secondary
                                    @click="openFailureItem(failure.itemId)"
                                >
                                    <template #icon><n-icon><EyeOutline /></n-icon></template>
                                    在 Eagle 中定位
                                </n-button>
                                <n-button size="small" quaternary circle title="删除记录" @click="clearFailure(failure.id)">
                                    <template #icon><n-icon><TrashOutline /></n-icon></template>
                                </n-button>
                            </div>
                        </article>
                    </div>
                </n-tab-pane>
            </n-tabs>
        </n-drawer-content>
    </n-drawer>
</template>

<style scoped>
    .task-entry {
        position: fixed;
        right: 18px;
        bottom: 18px;
        z-index: 1000;
        display: flex;
        align-items: center;
        gap: 8px;
    }
    .active-summary {
        width: 240px;
        min-height: 40px;
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto;
        gap: 2px 10px;
        padding: 6px 10px;
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 6px;
        background: #202124;
        color: #f1f3f4;
        text-align: left;
        cursor: pointer;
    }
    .active-summary span {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .active-summary span:nth-child(2) {
        grid-row: 2;
        color: #aeb4bc;
        font-size: 12px;
    }
    .active-summary strong {
        grid-column: 2;
        grid-row: 1 / span 2;
        align-self: center;
        font-size: 12px;
    }
    .task-list { display: grid; gap: 10px; }
    .task-toolbar { display: flex; justify-content: flex-end; margin-bottom: 10px; }
    .task-item {
        padding: 12px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
    }
    .task-header { display: flex; justify-content: space-between; gap: 12px; }
    .task-detail, .task-count { margin-top: 6px; color: #aeb4bc; font-size: 12px; }
    .task-error { margin-top: 8px; color: #e88080; overflow-wrap: anywhere; }
    .failure-path { margin-top: 6px; color: #aeb4bc; font-size: 12px; overflow-wrap: anywhere; }
    .failure-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 10px; }
    .task-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 10px; }
    .indeterminate { margin-top: 8px; }
</style>

