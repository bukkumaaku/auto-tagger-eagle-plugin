declare global {
    // 你原有的 Eagle 和 i18n 声明
    const i18next: any;
    const eagle: {
        plugin: any;
        library: any;
        window: any;
        app: any;
        log: any;
        item: any;
        os: any;
        extraModule: any;
        dialog: any;
    };

    // 👇 新增：告诉 TS 全局有 __dirname 这个变量
    const __dirname: string;

    // 👇 新增：利用 TS 的“接口合并”特性，给原生的 Window 对象加上 require 方法
    interface Window {
        require: (module: string) => any;
        eagle: any;
        pluginPath?: string;
    }
    const require: (module: string) => any;
}

// 必须保留这个 export {}，它让这个文件成为一个模块，从而激活 declare global
export {};
