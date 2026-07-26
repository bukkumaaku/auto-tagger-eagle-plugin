import { copyFile, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { resolveWorkerPlatform } from "./worker-platform.mjs";

const platform = resolveWorkerPlatform();
const manifestPath = path.resolve("dist", "manifest.json");
const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
manifest.platform = platform.eaglePlatform;
manifest.arch = platform.architecture;
await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");

const localeDirectory = path.resolve("dist", "_locales");
await rm(localeDirectory, { recursive: true, force: true });
await mkdir(localeDirectory, { recursive: true });
for (const language of manifest.languages ?? []) {
    await copyFile(
        path.resolve("_locales", `${language}.json`),
        path.join(localeDirectory, `${language}.json`),
    );
}

console.log(
    `prepared dist manifest for ${platform.eaglePlatform}-${platform.architecture}`,
);
