import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import { resolveWorkerPlatform } from "./worker-platform.mjs";

const platform = resolveWorkerPlatform();
const manifestPath = path.resolve("dist", "manifest.json");
const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
manifest.platform = platform.eaglePlatform;
manifest.arch = platform.architecture;
await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");

console.log(
    `prepared dist manifest for ${platform.eaglePlatform}-${platform.architecture}`,
);
