import { computed, reactive } from "vue";

export type TaskKind = "wd" | "llm" | "embedding" | "search" | "download";
export type TaskStatus = "running" | "paused" | "completed" | "failed" | "cancelled";

export type TaskRecord = {
    id: string;
    kind: TaskKind;
    title: string;
    detail: string;
    status: TaskStatus;
    completed: number;
    total: number;
    controllable: boolean;
    startedAt: number;
    finishedAt?: number;
    error?: string;
};

export type FailureRecord = {
    id: string;
    taskId: string;
    kind: TaskKind;
    itemId?: string;
    name: string;
    path: string;
    error: string;
    occurredAt: number;
};

const state = reactive({
    activeTaskId: "",
    tasks: [] as TaskRecord[],
    failures: [] as FailureRecord[],
});

let sequence = 0;
const controls = new Map<string, { paused: boolean; cancelRequested: boolean }>();

export class TaskCancelledError extends Error {
    constructor() {
        super("任务已取消");
        this.name = "TaskCancelledError";
    }
}

export const activeTask = computed(
    () => state.tasks.find((task) => task.id === state.activeTaskId) ?? null,
);
export const taskHistory = computed(() => state.tasks);
export const isTaskRunning = computed(() => Boolean(activeTask.value));
export const failureRecords = computed(() => state.failures);

export function beginTask(kind: TaskKind, title: string, total = 0, controllable = true) {
    if (activeTask.value) return null;
    const task: TaskRecord = {
        id: `task-${Date.now()}-${++sequence}`,
        kind,
        title,
        detail: "准备中",
        status: "running",
        completed: 0,
        total: Math.max(0, total),
        controllable,
        startedAt: Date.now(),
    };
    state.tasks.unshift(task);
    state.tasks.splice(20);
    state.activeTaskId = task.id;
    controls.set(task.id, { paused: false, cancelRequested: false });
    return task.id;
}

export function pauseTask(taskId: string) {
    const control = controls.get(taskId);
    const task = state.tasks.find((candidate) => candidate.id === taskId);
    if (!control || !task || task.status !== "running") return;
    control.paused = true;
    task.status = "paused";
    task.detail = "当前步骤完成后暂停";
}

export function resumeTask(taskId: string) {
    const control = controls.get(taskId);
    const task = state.tasks.find((candidate) => candidate.id === taskId);
    if (!control || !task || task.status !== "paused") return;
    control.paused = false;
    task.status = "running";
    task.detail = "继续执行";
}

export function requestTaskCancel(taskId: string) {
    const control = controls.get(taskId);
    const task = state.tasks.find((candidate) => candidate.id === taskId);
    if (!control || !task || !["running", "paused"].includes(task.status)) return;
    control.cancelRequested = true;
    control.paused = false;
    task.status = "running";
    task.detail = "当前步骤完成后取消";
}

export async function waitForTaskControl(taskId: string) {
    const control = controls.get(taskId);
    if (!control) return;
    while (control.paused && !control.cancelRequested) {
        await new Promise((resolve) => setTimeout(resolve, 100));
    }
    if (control.cancelRequested) throw new TaskCancelledError();
}

export function isTaskCancelled(error: unknown): error is TaskCancelledError {
    return error instanceof TaskCancelledError;
}

export function updateTask(
    taskId: string,
    update: Partial<Pick<TaskRecord, "title" | "detail" | "status" | "completed" | "total">>,
) {
    const task = state.tasks.find((candidate) => candidate.id === taskId);
    if (!task || (task.status !== "running" && task.status !== "paused")) return;
    Object.assign(task, update);
    task.completed = Math.max(0, task.completed);
    task.total = Math.max(0, task.total);
}

export function completeTask(taskId: string, detail = "已完成") {
    finishTask(taskId, "completed", detail);
}

export function failTask(taskId: string, error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    finishTask(taskId, "failed", "执行失败", message);
}

export function cancelTask(taskId: string) {
    finishTask(taskId, "cancelled", "已取消");
}

export function clearFinishedTasks() {
    state.tasks = state.tasks.filter(
        (task) => task.status === "running" || task.status === "paused",
    );
}

export function recordFailure(
    failure: Omit<FailureRecord, "id" | "occurredAt">,
) {
    state.failures = state.failures.filter(
        (existing) =>
            existing.kind !== failure.kind ||
            existing.itemId !== failure.itemId ||
            existing.path !== failure.path ||
            existing.error !== failure.error,
    );
    state.failures.unshift({
        ...failure,
        id: `failure-${Date.now()}-${++sequence}`,
        occurredAt: Date.now(),
    });
    state.failures.splice(500);
}

export function clearFailure(failureId: string) {
    state.failures = state.failures.filter((failure) => failure.id !== failureId);
}

export function clearFailures() {
    state.failures = [];
}

function finishTask(
    taskId: string,
    status: "completed" | "failed" | "cancelled",
    detail: string,
    error?: string,
) {
    const task = state.tasks.find((candidate) => candidate.id === taskId);
    if (!task || (task.status !== "running" && task.status !== "paused")) return;
    task.status = status;
    task.detail = detail;
    task.error = error;
    task.finishedAt = Date.now();
    if (status === "completed" && task.total > 0) task.completed = task.total;
    if (state.activeTaskId === taskId) state.activeTaskId = "";
    controls.delete(taskId);
}
