declare const require: (moduleName: string) => any;

const fs = require("node:fs");
const path = require("node:path");

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

const backupDirectory = (modelFilePath: string) =>
    path.join(path.dirname(modelFilePath), "backup");

const safeOperationName = (operation: TaggerBackup["operation"]) =>
    operation.replace(/[^a-z0-9-]/gi, "-");

export function createTaggerBackup(
    modelFilePath: string,
    operation: TaggerBackup["operation"],
    items: any[],
): string {
    if (!modelFilePath.trim()) throw new Error("模型路径为空，无法创建备份");
    const directory = backupDirectory(modelFilePath);
    fs.mkdirSync(directory, { recursive: true });
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
    const target = path.join(directory, filename);
    fs.writeFileSync(target, JSON.stringify(backup, null, 2), "utf8");
    return target;
}

export function listTaggerBackups(modelFilePath: string): TaggerBackupOption[] {
    if (!modelFilePath.trim()) return [];
    const directory = backupDirectory(modelFilePath);
    if (!fs.existsSync(directory)) return [];
    return fs
        .readdirSync(directory, { withFileTypes: true })
        .filter((entry: any) => entry.isFile() && entry.name.toLowerCase().endsWith(".json"))
        .map((entry: any) => {
            const value = path.join(directory, entry.name);
            try {
                const backup = readBackup(value);
                return {
                    label: `${backup.createdAt} · ${backup.operation} · ${backup.items.length} 项`,
                    value,
                    createdAt: backup.createdAt,
                    itemCount: backup.items.length,
                } satisfies TaggerBackupOption;
            } catch {
                return null;
            }
        })
        .filter((item: TaggerBackupOption | null): item is TaggerBackupOption => item !== null)
        .sort((left: TaggerBackupOption, right: TaggerBackupOption) =>
            right.createdAt.localeCompare(left.createdAt),
        );
}

export async function restoreTaggerBackup(
    backupPath: string,
    getItemById: (id: string) => Promise<any>,
): Promise<{ restored: number; skipped: number }> {
    const backup = readBackup(backupPath);
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

function readBackup(backupPath: string): TaggerBackup {
    const parsed = JSON.parse(fs.readFileSync(backupPath, "utf8"));
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
