import { chmod, copyFile, mkdir, stat } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const profile = process.argv[2];
if (profile !== "debug" && profile !== "release") {
    throw new Error("usage: bun scripts/copy-worker.mjs <debug|release>");
}

const platformNames = {
    win32: "win",
    darwin: "mac",
    linux: "linux",
};
const architectureNames = {
    x64: "x64",
    arm64: "arm64",
};

const platform = platformNames[process.platform];
const architecture = architectureNames[process.arch];
if (!platform || !architecture) {
    throw new Error(
        `unsupported worker platform: ${process.platform}-${process.arch}`,
    );
}

const executable = process.platform === "win32" ? "ai-worker.exe" : "ai-worker";
const source = path.resolve("backend", "target", profile, executable);
const destinationDirectory = path.resolve(
    "bin",
    `${platform}-${architecture}`,
);
const destination = path.join(destinationDirectory, executable);

await stat(source).catch(() => {
    throw new Error(`compiled ai-worker was not found at ${source}`);
});
await mkdir(destinationDirectory, { recursive: true });
await copyFile(source, destination);

if (process.platform === "win32") {
    const runtimeLibraries = ["DirectML.dll"];
    for (const library of runtimeLibraries) {
        const librarySource = path.resolve("backend", "target", profile, library);
        await stat(librarySource).catch(() => {
            throw new Error(`worker runtime library was not found at ${librarySource}`);
        });
        await copyFile(
            librarySource,
            path.join(destinationDirectory, library),
        );
    }
}

if (process.platform !== "win32") {
    await chmod(destination, 0o755);
}

console.log(`copied ${profile} worker to ${destination}`);
