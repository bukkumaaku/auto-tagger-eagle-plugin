import { shallowRef } from "vue";
import type {
    EmbeddingStatusResult,
    RemoteEmbeddingProfile,
} from "../protocol";
import { config } from "../api/backen";
import { getBackendClient } from "./backendClient";

const SESSION_ID = "embedding-main";
const INDEX_FILENAME = "semantic-index.sqlite3";
const CACHE_MAX_AGE_MS = 30_000;
const path = require("node:path");
const crypto = require("node:crypto");

export type SemanticIndexStatusTarget = {
    databasePath: string;
    namespace: string;
    modelKey: string;
    dimension: number;
    legacyModelKey: string;
};

export type SemanticIndexStatusSnapshot = {
    key: string;
    target: SemanticIndexStatusTarget;
    result: EmbeddingStatusResult;
    updatedAt: number;
};

export const semanticIndexStatus = shallowRef<SemanticIndexStatusSnapshot | null>(null);

const pendingRequests = new Map<string, Promise<EmbeddingStatusResult>>();

function targetKey(target: SemanticIndexStatusTarget) {
    return [
        target.databasePath,
        target.namespace,
        target.modelKey,
        target.dimension,
        target.legacyModelKey,
    ].join("\u0000");
}

export function getCachedSemanticIndexStatus(
    target: SemanticIndexStatusTarget,
    maxAgeMs = CACHE_MAX_AGE_MS,
) {
    const snapshot = semanticIndexStatus.value;
    if (!snapshot || snapshot.key !== targetKey(target)) return null;
    if (Date.now() - snapshot.updatedAt > maxAgeMs) return null;
    return snapshot.result;
}

export function cacheSemanticIndexStatus(
    target: SemanticIndexStatusTarget,
    result: EmbeddingStatusResult,
) {
    semanticIndexStatus.value = {
        key: targetKey(target),
        target,
        result,
        updatedAt: Date.now(),
    };
}

export function invalidateSemanticIndexStatus() {
    semanticIndexStatus.value = null;
}

export async function fetchSemanticIndexStatus(
    target: SemanticIndexStatusTarget,
    force = false,
) {
    if (!force) {
        const cached = getCachedSemanticIndexStatus(target);
        if (cached) return cached;
    }

    const key = targetKey(target);
    const pending = pendingRequests.get(key);
    if (pending) return pending;

    const request = getBackendClient()
        .embeddingStatus(SESSION_ID, target)
        .then((result) => {
            cacheSemanticIndexStatus(target, result);
            return result;
        })
        .finally(() => {
            pendingRequests.delete(key);
        });
    pendingRequests.set(key, request);
    return request;
}

export async function preloadSemanticIndexStatus() {
    try {
        const target = await resolveSemanticIndexStatusTarget();
        if (target) await fetchSemanticIndexStatus(target);
    } catch (error) {
        console.warn("预加载语义索引状态失败", error);
    }
}

async function resolveSemanticIndexStatusTarget(): Promise<SemanticIndexStatusTarget | null> {
    if (!config.modelLocation) return null;

    const backend = getBackendClient();
    const localModels = (await backend.scanEmbeddingModels(config.modelLocation)).models;
    const profiles: RemoteEmbeddingProfile[] = config.embeddingRemoteProfiles?.length
        ? config.embeddingRemoteProfiles
        : config.embeddingModelName && config.endpoint
          ? [{
                id: "legacy",
                name: "远程接口",
                provider: config.embeddingProvider === "gemini" ? "gemini" : "open_ai",
                endpoint: config.endpoint,
                apiKey: config.apiKey,
                model: config.embeddingModelName,
                dimension: config.embeddingDimension,
            }]
          : [];
    const models = [
        ...localModels.map((model) => ({
            selectionKey: model.modelKey,
            modelKey: model.modelKey,
            dimension: model.dimension,
            legacyModelKey: "",
        })),
        ...profiles
            .filter((profile) => profile.model && profile.endpoint)
            .map((profile) => {
                const dimension = profile.provider === "gemini" ? profile.dimension : 0;
                return {
                    selectionKey: `remote:${profile.id}`,
                    modelKey: remoteModelKey(profile.provider, profile.model, dimension),
                    dimension,
                    legacyModelKey: endpointRemoteModelKey(
                        profile.provider,
                        profile.endpoint,
                        profile.model,
                        dimension,
                    ),
                };
            }),
    ];
    const model = models.find((item) => item.selectionKey === config.embeddingModelId) || models[0];
    if (!model) return null;

    return {
        databasePath: path.join(config.modelLocation, "embedding", INDEX_FILENAME),
        namespace: await resolveLibraryNamespace(),
        modelKey: model.modelKey,
        dimension: model.dimension,
        legacyModelKey: model.legacyModelKey,
    };
}

function remoteModelKey(provider: "open_ai" | "gemini", model: string, dimension: number) {
    const identity = [provider, model.trim(), dimension].join("\u0000");
    const digest = crypto.createHash("sha256").update(identity, "utf8").digest("hex").slice(0, 16);
    return `${provider}:${model}:${dimension || "auto"}:${digest}`;
}

function endpointRemoteModelKey(
    provider: "open_ai" | "gemini",
    endpoint: string,
    model: string,
    dimension: number,
) {
    const identity = [provider, endpoint.trim().replace(/\/+$/, ""), model.trim(), dimension].join("\u0000");
    const digest = crypto.createHash("sha256").update(identity, "utf8").digest("hex").slice(0, 16);
    return `${provider}:${model}:${dimension || "auto"}:${digest}`;
}

async function resolveLibraryNamespace(): Promise<string> {
    let info: Record<string, unknown> | undefined;
    if (typeof eagle.library?.info === "function") {
        try {
            const result = await eagle.library.info();
            if (result && typeof result === "object") info = result as Record<string, unknown>;
        } catch (error) {
            console.warn("读取 Eagle 图库信息失败", error);
        }
    }
    const candidates = [
        info?.path,
        info?.libraryPath,
        eagle.library?.path,
        info?.name,
        eagle.library?.name,
    ];
    for (const candidate of candidates) {
        if (typeof candidate === "string" && candidate.trim()) return candidate.trim();
    }
    throw new Error("无法读取 Eagle 当前图库路径");
}
