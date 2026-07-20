import { getBackendClient } from "./backendClient";

export type TaggerBackupItem = {
    id: string;
    name: string;
    tags: string[];
    annotation: string;
};

export type TaggerBackup = {
    version: 1;
    createdAt: string;
    operation: "wd" | "llm-tag" | "llm-annotation";
    source?: TaggerBackupSource;
    items: TaggerBackupItem[];
};

export type TaggerBackupSource = "eagle" | "pixcall";

export type TaggerBackupOption = {
    label: string;
    value: string;
    createdAt: string;
    itemCount: number;
};

const backupDirectory = (modelFilePath: string, source?: TaggerBackupSource) => {
    const separator = modelFilePath.includes("\\") ? "\\" : "/";
    const index = Math.max(modelFilePath.lastIndexOf("/"), modelFilePath.lastIndexOf("\\"));
    const parent = index >= 0 ? modelFilePath.slice(0, index) : modelFilePath;
    const directory = `${parent}${separator}backup`;
    return source ? `${directory}${separator}${source}` : directory;
};

export const scopedTaggerBackupDirectory = (
    root: string,
    source: TaggerBackupSource,
    category: "llm",
) => {
    if (!root.trim()) throw new Error("模型目录为空，无法定位备份目录");
    const separator = root.includes("\\") ? "\\" : "/";
    return [root.replace(/[\\/]+$/g, ""), "backup", source, category].join(separator);
};

const safeOperationName = (operation: TaggerBackup["operation"]) =>
    operation.replace(/[^a-z0-9-]/gi, "-");

export async function createTaggerBackup(
    modelFilePath: string,
    operation: TaggerBackup["operation"],
    items: any[],
    source?: TaggerBackupSource,
): Promise<string> {
    if (!modelFilePath.trim()) throw new Error("模型路径为空，无法创建备份");
    return createTaggerBackupInDirectory(
        backupDirectory(modelFilePath, source),
        operation,
        items,
        source,
    );
}

export async function createTaggerBackupInDirectory(
    directory: string,
    operation: TaggerBackup["operation"],
    items: any[],
    source?: TaggerBackupSource,
): Promise<string> {
    if (!directory.trim()) throw new Error("备份目录为空，无法创建备份");
    const createdAt = new Date().toISOString();
    const stamp = createdAt.replace(/[:.]/g, "-");
    const suffix = Math.random().toString(36).slice(2, 8);
    const filename = `${stamp}-${safeOperationName(operation)}-${suffix}.json`;
    const backup: TaggerBackup = {
        version: 1,
        createdAt,
        operation,
        source,
        items: items.map((item) => ({
            id: String(item.id),
            name: String(item.name || item.id || ""),
            tags: Array.isArray(item.tags)
                ? item.tags.map((tag: unknown) => String(tag)).filter(Boolean)
                : [],
            annotation: String(item.annotation || ""),
        })),
    };
    const result = await getBackendClient().writeBackup(
        directory,
        filename,
        JSON.stringify(backup, null, 2),
    );
    return result.path;
}

export async function listTaggerBackups(
    modelFilePath: string,
    source?: TaggerBackupSource,
): Promise<TaggerBackupOption[]> {
    if (!modelFilePath.trim()) return [];
    return listTaggerBackupsInDirectory(backupDirectory(modelFilePath, source));
}

export async function listTaggerBackupsInDirectory(
    directory: string,
): Promise<TaggerBackupOption[]> {
    if (!directory.trim()) return [];
    const result = await getBackendClient().listBackups(directory);
    const options = await Promise.all(result.entries.map(async (entry) => {
            try {
                const backup = await readTaggerBackup(entry.path);
                return {
                    label: `${backup.createdAt} · ${backup.operation} · ${backup.items.length} 项`,
                    value: entry.path,
                    createdAt: backup.createdAt,
                    itemCount: backup.items.length,
                } satisfies TaggerBackupOption;
            } catch {
                return null;
            }
        }));
    return options.filter((item): item is TaggerBackupOption => item !== null)
        .sort((left: TaggerBackupOption, right: TaggerBackupOption) =>
            right.createdAt.localeCompare(left.createdAt),
        );
}

export async function filterCompatibleTaggerBackups(
    options: TaggerBackupOption[],
    source: TaggerBackupSource,
    getItemById: (id: string) => Promise<any>,
): Promise<TaggerBackupOption[]> {
    const checked = await Promise.all(
        options.map(async (option) => {
            try {
                const backup = await readTaggerBackup(option.value);
                if (backup.source) return backup.source === source ? option : null;
                for (const item of backup.items.slice(0, 5)) {
                    try {
                        if (await getItemById(item.id)) return option;
                    } catch {
                        // Try another item from legacy backups before excluding it.
                    }
                }
            } catch {
                // Invalid backups are excluded from the selector.
            }
            return null;
        }),
    );
    return checked.filter((item): item is TaggerBackupOption => item !== null);
}

export function mergeTaggerBackupOptions(
    ...groups: TaggerBackupOption[][]
): TaggerBackupOption[] {
    const merged = new Map<string, TaggerBackupOption>();
    for (const option of groups.flat()) merged.set(option.value, option);
    return Array.from(merged.values()).sort((left, right) =>
        right.createdAt.localeCompare(left.createdAt),
    );
}

export async function restoreTaggerBackup(
    backupPath: string,
    getItemById: (id: string) => Promise<any>,
    expectedSource?: TaggerBackupSource,
): Promise<{ restored: number; skipped: number }> {
    const backup = await readTaggerBackup(backupPath);
    if (backup.source && expectedSource && backup.source !== expectedSource) {
        throw new Error("该备份属于另一个宿主，已取消恢复");
    }
    let restored = 0;
    let skipped = 0;
    for (const saved of backup.items) {
        try {
            const item = await getItemById(saved.id);
            if (!item) {
                skipped++;
                continue;
            }
            item.tags = [...saved.tags];
            item.annotation = saved.annotation;
            await item.save();
            restored++;
        } catch {
            skipped++;
        }
    }
    return { restored, skipped };
}

export async function readTaggerBackup(backupPath: string): Promise<TaggerBackup> {
    const result = await getBackendClient().readBackup(backupPath);
    const parsed = JSON.parse(result.content);
    if (
        !parsed ||
        parsed.version !== 1 ||
        !Array.isArray(parsed.items) ||
        typeof parsed.createdAt !== "string"
    ) {
        throw new Error("备份文件格式无效");
    }
    return parsed as TaggerBackup;
}
