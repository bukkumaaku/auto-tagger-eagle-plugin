# Eagle 自动打标插件

📌 为 Eagle 资源库中有缩略图的文件自动生成标签，支持 GPU 加速（directML、webGPU）和 CPU 回退模式。

## 功能特性

-   自动为图片/视频文件生成描述性标签
-   优先使用 GPU 加速（支持 DirectML/WebGPU 等非 CUDA 加速）
-   无 GPU 时自动切换至 CPU 模式
-   支持 WDv2 和 Vitv3 系列模型

## 安装指南

### 系统要求

-   Windows x86/x64（推荐）
-   macOS（需手动配置）
-   Eagle v4.0

## 安装方法

### 快速安装（推荐）

```bash
git clone https://github.com/bukkumaaku/auto-tagger-eagle-plugin.git
cd auto-tagger-eagle-plugin
```

▸ **Windows 用户**

-   安装[nodejs](https://nodejs.org/zh-cn/download)
-   双击运行 `install.bat`

▸ **Mac 用户**
双击运行 `install.command`（首次运行需在终端执行授权：`chmod +x install.command`）

### 手动安装

#### 1. 克隆仓库

```bash
git clone https://github.com/bukkumaaku/auto-tagger-eagle-plugin.git
cd auto-tagger-eagle-plugin
```

#### 2. 环境准备

```bash
# 安装项目依赖
npm install --registry https://registry.npmmirror.com
```

#### 系统特定说明

-   **Windows**：需提前安装 [Node.js](https://nodejs.org) 以获得 npm 环境
-   **Mac**：自带 npm 环境，若提示权限错误可在命令前加 `sudo`

## 模型配置

### 获取预训练模型

1. **推荐方式（Git）**

    ```bash
    cd models
    git clone https://huggingface.co/SmilingWolf/wd-v1-4-moat-tagger-v2
    git clone https://huggingface.co/SmilingWolf/wd-vit-tagger-v3
    ```

2. **手动下载**
    - 访问 [SmilingWolf 模型库](https://huggingface.co/SmilingWolf)
    - 下载目标模型的 `model.onnx` 和 `selected_tags.csv`
    - 在 `models` 目录新建模型文件夹存放

### 目录结构

```
auto-tagger-eagle-plugin/
└── models/
    ├── wd-v1-4-moat-tagger-v2/
    │   ├── model.onnx
    │   └── selected_tags.csv
    └── wd-vit-tagger-v3/
        ├── model.onnx
        └── selected_tags.csv
```

## 使用说明

1. eagle中加载本地插件
2. 选择多个文件
3. 按照需求自己填写配置
4. 在文件信息面板查看生成标签

## 注意事项

⚠️ 当前已验证模型：

-   `wd-v1-4-moat-tagger-v2`
-   `wd-vit-tagger-v3`

## 常见问题

❓ **性能优化建议**

-   推荐使用 ≥4GB 显存的 GPU

❓ **模型兼容性问题**
请在 Issues 提交：

1. 模型名称
2. 错误日志截图
3. 硬件配置信息

## 贡献与许可

欢迎通过 Issues 提交：

-   新模型适配需求
-   多语言支持
-   性能优化建议
