import {
    chmod,
    copyFile,
    mkdir,
    readdir,
    rm,
    stat,
} from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import { resolveWorkerPlatform } from "./worker-platform.mjs";

const profile = process.argv[2];
if (profile !== "debug" && profile !== "release") {
    throw new Error("usage: bun scripts/copy-worker.mjs <debug|release>");
}

const platform = resolveWorkerPlatform();
const sourceDirectory = path.resolve(
    "backend",
    "target",
    ...(platform.target ? [platform.target] : []),
    profile,
);
const source = path.join(sourceDirectory, platform.executable);
const destinationDirectory = path.resolve("bin", platform.directory);
const destination = path.join(destinationDirectory, platform.executable);

await stat(source).catch(() => {
    throw new Error(`compiled ai-worker was not found at ${source}`);
});
await rm(destinationDirectory, { recursive: true, force: true });
await mkdir(destinationDirectory, { recursive: true });
await copyFile(source, destination);

if (platform.nodePlatform === "win32") {
    const directMl = path.join(sourceDirectory, "DirectML.dll");
    await stat(directMl).catch(() => {
        throw new Error(`worker runtime library was not found at ${directMl}`);
    });
    await copyFile(directMl, path.join(destinationDirectory, "DirectML.dll"));
}

if (platform.nodePlatform === "darwin") {
    const runtimeLibraries = (await readdir(sourceDirectory)).filter((name) =>
        name.endsWith(".dylib"),
    );
    for (const library of runtimeLibraries) {
        await copyFile(
            path.join(sourceDirectory, library),
            path.join(destinationDirectory, library),
        );
    }
    await chmod(destination, 0o755);
}

console.log(`copied ${profile} worker to ${destination}`);
