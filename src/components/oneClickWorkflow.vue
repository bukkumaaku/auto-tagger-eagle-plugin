<script setup lang="ts">
    import {
        NAlert,
        NButton,
        NIcon,
        NProgress,
        NRadioButton,
        NRadioGroup,
        NTag,
    } from "naive-ui";
    import {
        CheckmarkCircleOutline,
        CloseCircleOutline,
        EllipseOutline,
        FlashOutline,
        SyncOutline,
    } from "@vicons/ionicons5";
    import { computed, ref } from "vue";
    import { backenAPI, notification } from "../api/backen";
    import llmTypeTagger from "./llmTypeTagger.vue";
    import semanticSearch from "./semanticSearch.vue";
    import wdTypeTagger from "./wdTypeTagger.vue";

    type WorkflowItem = {
        id: string;
        name?: string;
        [key: string]: unknown;
    };
    type TaggerMode = "wd" | "llm";
    type StageStatus = "pending" | "running" | "done" | "failed";
    type StageKey =
        | "tagging"
        | "annotation"
        | "imageEmbedding"
        | "tagEmbedding"
        | "annotationEmbedding";
    type WorkflowEngine = {
        ready: boolean;
    };
    type WdEngine = WorkflowEngine & {
        runForItems: (items: WorkflowItem[]) => Promise<void>;
    };
    type LlmEngine = WorkflowEngine & {
        runForItems: (
            items: WorkflowItem[],
            mode: "tag" | "annotation",
        ) => Promise<void>;
    };
    type SemanticEngine = WorkflowEngine & {
        runImageIndexing: (items: WorkflowItem[]) => Promise<void>;
        runTagIndexing: (items: WorkflowItem[]) => Promise<void>;
        runAnnotationIndexing: (items: WorkflowItem[]) => Promise<void>;
    };

    const stageDefinitions: Array<{ key: StageKey; label: string }> = [
        { key: "tagging", label: "图片打标" },
        { key: "annotation", label: "生成注释" },
        { key: "imageEmbedding", label: "图片向量化" },
        { key: "tagEmbedding", label: "标签向量化" },
        { key: "annotationEmbedding", label: "注释向量化" },
    ];
    const taggerMode = ref<TaggerMode>("wd");
    const isRunning = ref(false);
    const selectedCount = ref(0);
    const errorText = ref("");
    const stages = ref<Record<StageKey, StageStatus>>(createInitialStages());
    const wdRef = ref<WdEngine | null>(null);
    const llmRef = ref<LlmEngine | null>(null);
    const semanticRef = ref<SemanticEngine | null>(null);

    const completedStages = computed(
        () => Object.values(stages.value).filter((status) => status === "done").length,
    );
    const progress = computed(() =>
        Math.round((completedStages.value / stageDefinitions.length) * 100),
    );

    function createInitialStages(): Record<StageKey, StageStatus> {
        return {
            tagging: "pending",
            annotation: "pending",
            imageEmbedding: "pending",
            tagEmbedding: "pending",
            annotationEmbedding: "pending",
        };
    }

    function stageLabel(status: StageStatus) {
        return {
            pending: "等待",
            running: "处理中",
            done: "完成",
            failed: "失败",
        }[status];
    }

    function stageTagType(status: StageStatus) {
        if (status === "done") return "success" as const;
        if (status === "failed") return "error" as const;
        if (status === "running") return "info" as const;
        return "default" as const;
    }

    function stageIcon(status: StageStatus) {
        if (status === "done") return CheckmarkCircleOutline;
        if (status === "failed") return CloseCircleOutline;
        if (status === "running") return SyncOutline;
        return EllipseOutline;
    }

    async function waitForEngines() {
        const deadline = Date.now() + 20_000;
        while (Date.now() < deadline) {
            if (
                wdRef.value?.ready &&
                llmRef.value?.ready &&
                semanticRef.value?.ready
            ) {
                return;
            }
            await new Promise((resolve) => setTimeout(resolve, 100));
        }
        throw new Error("处理模块初始化超时，请检查模型和远程接口配置");
    }

    async function runStage(key: StageKey, action: () => Promise<void>) {
        stages.value[key] = "running";
        try {
            await action();
            stages.value[key] = "done";
        } catch (error) {
            stages.value[key] = "failed";
            throw error;
        }
    }

    async function refreshItems(snapshot: WorkflowItem[]) {
        return Promise.all(
            snapshot.map(async (snapshotItem) => {
                const item = await eagle.item.getById(snapshotItem.id);
                if (!item) {
                    throw new Error(`无法重新读取图片 ${snapshotItem.name || snapshotItem.id}`);
                }
                return item as WorkflowItem;
            }),
        );
    }

    async function startWorkflow() {
        if (isRunning.value || backenAPI.is_processing) {
            notification("仍有任务正在进行中，请等待", "warning");
            return;
        }

        const snapshot = (await eagle.item.getSelected()) as WorkflowItem[];
        if (snapshot.length === 0) {
            notification("请先选择要处理的图片", "warning");
            return;
        }

        selectedCount.value = snapshot.length;
        stages.value = createInitialStages();
        errorText.value = "";
        isRunning.value = true;
        try {
            await waitForEngines();
            await runStage("tagging", () =>
                taggerMode.value === "wd"
                    ? wdRef.value!.runForItems(snapshot)
                    : llmRef.value!.runForItems(snapshot, "tag"),
            );
            await runStage("annotation", () =>
                llmRef.value!.runForItems(snapshot, "annotation"),
            );

            const refreshedItems = await refreshItems(snapshot);
            await runStage("imageEmbedding", () =>
                semanticRef.value!.runImageIndexing(refreshedItems),
            );
            await runStage("tagEmbedding", () =>
                semanticRef.value!.runTagIndexing(refreshedItems),
            );
            await runStage("annotationEmbedding", () =>
                semanticRef.value!.runAnnotationIndexing(refreshedItems),
            );
            notification(`已完成 ${snapshot.length} 张图片的一键处理`, "success");
        } catch (error) {
            errorText.value = error instanceof Error ? error.message : String(error);
            notification(errorText.value, "error");
        } finally {
            isRunning.value = false;
        }
    }
</script>

<template>
    <main class="workflow-page">
        <header class="workflow-header">
            <div>
                <h1>一键处理</h1>
                <span v-if="selectedCount > 0" class="selection-count">
                    本次固定 {{ selectedCount }} 张图片
                </span>
            </div>
            <n-radio-group v-model:value="taggerMode" :disabled="isRunning">
                <n-radio-button value="wd">WD 打标</n-radio-button>
                <n-radio-button value="llm">LLM 打标</n-radio-button>
            </n-radio-group>
        </header>

        <section class="workflow-panel">
            <div class="progress-row">
                <n-progress
                    type="line"
                    :percentage="progress"
                    :status="errorText ? 'error' : 'default'"
                    :show-indicator="false"
                />
                <strong>{{ completedStages }}/{{ stageDefinitions.length }}</strong>
            </div>

            <ol class="stage-list">
                <li
                    v-for="(stage, index) in stageDefinitions"
                    :key="stage.key"
                    :class="`stage--${stages[stage.key]}`"
                >
                    <span class="stage-order">{{ index + 1 }}</span>
                    <n-icon
                        :component="stageIcon(stages[stage.key])"
                        :class="{ 'stage-icon--running': stages[stage.key] === 'running' }"
                        size="20"
                    />
                    <span class="stage-name">{{ stage.label }}</span>
                    <n-tag size="small" :type="stageTagType(stages[stage.key])">
                        {{ stageLabel(stages[stage.key]) }}
                    </n-tag>
                </li>
            </ol>

            <n-alert v-if="errorText" type="error" title="流程已停止">
                {{ errorText }}
            </n-alert>

            <div class="workflow-actions">
                <n-button
                    type="primary"
                    size="large"
                    :loading="isRunning"
                    :disabled="isRunning"
                    @click="startWorkflow"
                >
                    <template #icon><n-icon :component="FlashOutline" /></template>
                    开始处理选中图片
                </n-button>
            </div>
        </section>

        <div class="workflow-engines" aria-hidden="true">
            <wdTypeTagger ref="wdRef" engine-only />
            <llmTypeTagger ref="llmRef" />
            <semanticSearch ref="semanticRef" />
        </div>
    </main>
</template>

<style scoped>
    .workflow-page {
        width: min(820px, calc(100% - 48px));
        margin: 32px auto;
    }

    .workflow-header,
    .progress-row,
    .stage-list li,
    .workflow-actions {
        display: flex;
        align-items: center;
    }

    .workflow-header {
        justify-content: space-between;
        gap: 24px;
        margin-bottom: 22px;
    }

    .workflow-header h1 {
        margin: 0 0 5px;
        font-size: 24px;
        line-height: 1.2;
        letter-spacing: 0;
    }

    .selection-count {
        color: #8f989f;
        font-size: 13px;
    }

    .workflow-panel {
        border: 1px solid #2b3035;
        border-radius: 6px;
        background: #181b1e;
        padding: 24px;
    }

    .progress-row {
        gap: 16px;
        margin-bottom: 20px;
    }

    .progress-row strong {
        flex: none;
        min-width: 32px;
        color: #b6bdc3;
        font-size: 13px;
        text-align: right;
    }

    .stage-list {
        display: grid;
        gap: 1px;
        margin: 0 0 20px;
        padding: 0;
        list-style: none;
        overflow: hidden;
        border: 1px solid #2b3035;
        border-radius: 4px;
        background: #2b3035;
    }

    .stage-list li {
        min-height: 52px;
        gap: 12px;
        padding: 0 15px;
        background: #1d2023;
        color: #aeb6bc;
    }

    .stage-list li.stage--running {
        background: #20272b;
        color: #f0f3f5;
    }

    .stage-list li.stage--done {
        color: #b8d8c5;
    }

    .stage-list li.stage--failed {
        color: #e7aaaa;
    }

    .stage-order {
        width: 20px;
        color: #717b83;
        font-variant-numeric: tabular-nums;
        text-align: center;
    }

    .stage-name {
        flex: 1;
        min-width: 0;
        font-size: 14px;
    }

    .stage-icon--running {
        animation: workflow-spin 1s linear infinite;
    }

    .workflow-actions {
        justify-content: flex-end;
        margin-top: 20px;
    }

    .workflow-engines {
        display: none;
    }

    @keyframes workflow-spin {
        to { transform: rotate(360deg); }
    }

    @media (max-width: 640px) {
        .workflow-page {
            width: calc(100% - 24px);
            margin-top: 18px;
        }

        .workflow-header {
            align-items: stretch;
            flex-direction: column;
            gap: 14px;
        }

        .workflow-panel {
            padding: 16px;
        }

        .workflow-actions :deep(.n-button) {
            width: 100%;
        }
    }
</style>
