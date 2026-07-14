import process from "node:process";

const targetPlatforms = {
    "x86_64-pc-windows-msvc": {
        nodePlatform: "win32",
        eaglePlatform: "win32",
        architecture: "x64",
        directory: "win-x64",
        executable: "ai-worker.exe",
    },
    "aarch64-apple-darwin": {
        nodePlatform: "darwin",
        eaglePlatform: "darwin",
        architecture: "arm64",
        directory: "mac-arm64",
        executable: "ai-worker",
    },
};

const hostTargets = {
    "win32-x64": "x86_64-pc-windows-msvc",
    "darwin-arm64": "aarch64-apple-darwin",
};

export function resolveWorkerPlatform(
    target = process.env.CARGO_BUILD_TARGET?.trim(),
) {
    const resolvedTarget = target || hostTargets[`${process.platform}-${process.arch}`];
    const platform = targetPlatforms[resolvedTarget];
    if (!platform) {
        throw new Error(
            `unsupported worker target: ${resolvedTarget || `${process.platform}-${process.arch}`}`,
        );
    }
    return { ...platform, target: target ? resolvedTarget : null };
}
