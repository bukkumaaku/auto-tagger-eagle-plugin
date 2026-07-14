import { createApp } from "vue";
import "./style.css";
import App from "./App.vue";
import { getBackendClient } from "./services/backendClient";

/* // @ts-ignored
if (typeof window?.i18next === "undefined") {
    console.log("【开发预览模式】检测到无 i18next 环境，已注入全局伪造对象");
    (window as any).i18next = {
        // 伪造 t 方法，传入什么翻译键值，就直接原样返回什么
        t: (key: string) => key,
        // 如果你代码里还用到了改变语言的方法，也可以顺手 mock 一下
        changeLanguage: () => Promise.resolve(),
    };
} */

const app = createApp(App);
app.mount("#app");

if (typeof require === "function") {
    try {
        const backend = getBackendClient();
        backend.start();
        window.addEventListener("beforeunload", () => backend.dispose(), {
            once: true,
        });
    } catch (error) {
        console.error("Failed to start ai-worker", error);
    }
}
