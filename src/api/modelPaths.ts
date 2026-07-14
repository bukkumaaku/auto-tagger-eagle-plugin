import { config } from "./backen";

const path = require("node:path");

export type ModelCategory = "wd" | "llm" | "embedding" | "llamafile";

export function categoryDirectory(category: ModelCategory): string {
    return path.join(config.modelLocation || "", category);
}

export function modelDirectory(
    category: Exclude<ModelCategory, "llamafile">,
    modelName: string,
): string {
    return path.join(categoryDirectory(category), modelName);
}

export function modelResourcePath(
    category: Exclude<ModelCategory, "llamafile">,
    modelName: string,
    filename: string,
): string {
    return path.join(modelDirectory(category, modelName), filename);
}

export function llamafilePath(filename: string): string {
    return path.join(categoryDirectory("llamafile"), filename);
}
