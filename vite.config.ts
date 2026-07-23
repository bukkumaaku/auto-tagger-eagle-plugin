import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import electronrender from "vite-plugin-electron-renderer";
import { viteStaticCopy } from "vite-plugin-static-copy";
import { realpathSync } from "node:fs";

const projectRoot = realpathSync(process.cwd());

// https://vite.dev/config/
export default defineConfig(({ command }) => ({
    root: projectRoot,
    plugins: [
        vue(),
        electronrender(),
        viteStaticCopy({
            targets: [
                { src: "manifest.json", dest: "" },
                { src: "logo.png", dest: "" },
                { src: "package.json", dest: "" },
                { src: "_locales", dest: "" },
                { src: "tagset.csv", dest: "" },
                { src: "bin", dest: "" },
            ],
        }),
    ],
    base: command === "serve" ? "/" : "./",
    server: {
        port: 5173,
        strictPort: true,
        // 核心修正 2：强制告诉 Vite，所有热更新的资源图片、CSS 都必须从这里拿
        origin: "http://localhost:5173",
    },
    build: {
        outDir: "dist",
        rollupOptions: {
            external: [
                "child_process",
                "path",
                "fs",
                "crypto",
                "os",
                "readline",
                "url",
                "buffer",
            ],
        },
    },
    esbuild: {
        keepNames: true,
    },
}));
