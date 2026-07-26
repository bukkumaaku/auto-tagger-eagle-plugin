import { readFile, stat } from "node:fs/promises";
import path from "node:path";

import { resolveWorkerPlatform } from "./worker-platform.mjs";

const platform = resolveWorkerPlatform();
const distDirectory = path.resolve("dist");
const workerPath = path.join("bin", platform.directory, platform.executable);
const requiredFiles = ["index.html", "tagset.csv", workerPath];
if (platform.nodePlatform === "win32") {
    requiredFiles.push(path.join("bin", platform.directory, "DirectML.dll"));
}

for (const relativePath of requiredFiles) {
    const filePath = path.join(distDirectory, relativePath);
    const file = await stat(filePath).catch(() => null);
    if (!file?.isFile() || file.size === 0) {
        throw new Error(`required build artifact is missing or empty: ${filePath}`);
    }
}

if (platform.nodePlatform === "darwin") {
    const worker = await stat(path.join(distDirectory, workerPath));
    if ((worker.mode & 0o111) === 0) {
        throw new Error(`macOS worker is not executable: ${workerPath}`);
    }
}

const manifestPath = path.join(distDirectory, "manifest.json");
const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
if (manifest?.main?.url !== "./index.html") {
    throw new Error(
        `dist manifest must use ./index.html, got ${manifest?.main?.url}`,
    );
}
if (
    manifest?.platform !== platform.eaglePlatform ||
    manifest?.arch !== platform.architecture
) {
    throw new Error(
        `dist manifest target mismatch: expected ${platform.eaglePlatform}-${platform.architecture}, got ${manifest?.platform}-${manifest?.arch}`,
    );
}

if (!Array.isArray(manifest?.languages) || manifest.languages.length === 0) {
    throw new Error("dist manifest must declare at least one language");
}
if (!manifest.languages.includes(manifest.fallbackLanguage)) {
    throw new Error(
        `fallback language is not declared: ${manifest.fallbackLanguage}`,
    );
}
for (const language of manifest.languages) {
    const relativePath = path.join("_locales", `${language}.json`);
    const localePath = path.join(distDirectory, relativePath);
    const localeFile = await stat(localePath).catch(() => null);
    if (!localeFile?.isFile() || localeFile.size === 0) {
        throw new Error(
            `required locale artifact is missing or empty: ${localePath}`,
        );
    }
    JSON.parse(await readFile(localePath, "utf8"));
}

const packagePath = path.join(distDirectory, "package.json");
const packageManifest = JSON.parse(await readFile(packagePath, "utf8"));
const runtimeDependencies = Object.keys(packageManifest.dependencies ?? {});
if (runtimeDependencies.length > 0) {
    throw new Error(
        `dist package must not contain runtime dependencies: ${runtimeDependencies.join(", ")}`,
    );
}

for (const obsoletePath of [
    "node_modules",
    "js",
    "config.json",
    "config.json5",
]) {
    const artifactPath = path.join(distDirectory, obsoletePath);
    const artifact = await stat(artifactPath).catch(() => null);
    if (artifact) {
        throw new Error(`obsolete dist artifact must not exist: ${artifactPath}`);
    }
}

console.log(
    `verified ${platform.eaglePlatform}-${platform.architecture} dist artifacts`,
);
