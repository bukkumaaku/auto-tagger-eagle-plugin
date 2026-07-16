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
    items: TaggerBackupItem[];
};

export type TaggerBackupOption = {
    label: string;
    value: string;
    createdAt: string;
    itemCount: number;
};

const backupDirectory = (modelFilePath: string) => {
    const separator = modelFilePath.includes("\\") ? "\\" : "/";
    const index = Math.max(modelFilePath.lastIndexOf("/"), modelFilePath.lastIndexOf("\\"));
    const parent = index >= 0 ? modelFilePath.slice(0, index) : modelFilePath;
    return `${parent}${separator}backup`;
};

const safeOperationName = (operation: TaggerBackup["operation"]) =>
    operation.replace(/[^a-z0-9-]/gi, "-");

export async function createTaggerBackup(
    modelFilePath: string,
    operation: TaggerBackup["operation"],
    items: any[],
): Promise<string> {
    if (!modelFilePath.trim()) throw new Error("模型路径为空，无法创建备份");
    const createdAt = new Date().toISOString();
    const stamp = createdAt.replace(/[:.]/g, "-");
    const suffix = Math.random().toString(36).slice(2, 8);
    const filename = `${stamp}-${safeOperationName(operation)}-${suffix}.json`;
    const backup: TaggerBackup = {
        version: 1,
        createdAt,
        operation,
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
        backupDirectory(modelFilePath),
        filename,
        JSON.stringify(backup, null, 2),
    );
    return result.path;
}

export async function listTaggerBackups(modelFilePath: string): Promise<TaggerBackupOption[]> {
    if (!modelFilePath.trim()) return [];
    const result = await getBackendClient().listBackups(backupDirectory(modelFilePath));
    const options = await Promise.all(result.entries.map(async (entry) => {
            try {
                const backup = await readBackup(entry.path);
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

export async function restoreTaggerBackup(
    backupPath: string,
    getItemById: (id: string) => Promise<any>,
): Promise<{ restored: number; skipped: number }> {
    const backup = await readBackup(backupPath);
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

async function readBackup(backupPath: string): Promise<TaggerBackup> {
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
