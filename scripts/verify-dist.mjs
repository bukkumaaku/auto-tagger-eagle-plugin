import { readFile, stat } from "node:fs/promises";
import path from "node:path";

const distDirectory = path.resolve("dist");
const requiredFiles = [
    "index.html",
    "tagset.csv",
    path.join("bin", "win-x64", "ai-worker.exe"),
    path.join("bin", "win-x64", "DirectML.dll"),
];

for (const relativePath of requiredFiles) {
    const filePath = path.join(distDirectory, relativePath);
    const file = await stat(filePath).catch(() => null);
    if (!file?.isFile() || file.size === 0) {
        throw new Error(`required build artifact is missing or empty: ${filePath}`);
    }
}

const manifestPath = path.join(distDirectory, "manifest.json");
const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
if (manifest?.main?.url !== "./index.html") {
    throw new Error(
        `dist manifest must use ./index.html, got ${manifest?.main?.url}`,
    );
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
    "verified dist artifacts and absence of legacy JavaScript runtimes",
);
