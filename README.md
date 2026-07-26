# Eagle Auto Tagger

[English](./README.en.md) | 简体中文

Eagle Auto Tagger 是一个使用 Rust 后端和 Vue 界面的 Eagle 插件，用于图片与视频打标、视觉语言模型标签和注释生成、一键处理，以及图片、标签和注释的混合语义搜索。

当前版本：`2.2.1`

## 功能

### WD 模型打标

- 支持 PNG、JPG、JPEG、WEBP 和 BMP 图片，GIF 暂不处理。
- 支持图片批量打标。
- 可选中文、英文或中英文标签，并设置标签分隔符。
- 支持不覆写、覆写和合并已有标签。
- 可设置置信度阈值和批次大小。
- 单张图片读取失败时会跳过该图片，并在任务结果中列出失败文件。
- 视频可以选择不读取内容，或使用 ffmpeg 抽取 1%、20%、40%、60%、80% 和 99% 位置的画面后打标。
- 支持 WD、CL-Tagger 和 Camie 等模型格式，模型列表由后端扫描实际文件获得。

### LLM 图像理解

- 支持本地 llamafile + GGUF 视觉语言模型，以及 OpenAI Compatible 和 Gemini 原生 REST 远程视觉模型。
- 支持保存多套远程 LLM profile，并为远程批量处理设置并发数。
- 支持生成标签或写入 Eagle 注释。
- 标签提示词和注释提示词可以在界面中修改，并分别自动保存。
- 提供恢复默认提示词功能。
- 支持不覆写、覆写和合并已有标签或注释。
- 当前本地模型包括 Qwen3.5-9B、Qwen3-VL 8B、Llama JoyCaption Alpha Two 8B 和 Beta One 8B；模型需要同时具备 GGUF 主模型和 mmproj 文件。
- llamafile 的标准输出和错误输出按日期写入 `模型根目录/logs/YYYY-MM-DD.log`。
- 本地 LLM 支持自动检测 CUDA、Metal 和 Vulkan；后端不可用时可显示明确诊断并按设置回退到 Vulkan 或 CPU。

### 图片语义搜索

- 对 Eagle 图片建立向量索引，已经索引的图片会自动跳过。
- 支持暂停、继续、设置批次大小，以及查看索引进度。
- 支持以文字搜图和以图搜图。
- 搜索结果按相似度排序，并使用懒加载瀑布流显示。
- 单击结果可预览，双击结果可在 Eagle 中打开图片。
- 支持本地 Jina CLIP v2 模型。
- 支持 OpenAI Compatible 和 Gemini 原生 REST 远程向量接口。
- 支持保存多套远程 embedding profile。
- 图片、标签和注释分别建立向量索引，可以设置权重进行组合搜索。
- 支持正向和负向文字条件，并可与以图搜图结果融合。
- 不同模型、协议和向量维度使用独立的 SQLite 向量表，避免不同向量空间混合。
- 提供索引健康检查、重建恢复、过期项目清理和失败项目列表。

### 一键处理与备份

- 一键串联 WD 打标、LLM 标签、LLM 注释，以及图片、标签和注释向量化。
- 支持暂停、继续和取消长时间任务，并在任务中心查看分阶段进度与失败项。
- 在批量写入前创建标签或注释备份，并可按类别选择备份恢复。

### 任务和配置管理

- 统一任务中心显示打标、索引、搜索和下载任务。
- 模型下载支持实时进度和错误提示。
- 插件启动时会检查 GitHub Releases；发现新版本后提示用户，并可直接打开 Release 下载页面。
- 配置由 Rust 后端读写，默认文件位于用户目录下的 `.eagle-auto-tagger/config.json5`。
- 新增配置键会自动使用默认值写入 JSON5 文件。
- 远程 embedding 和 LLM profile 的 API key 使用操作系统凭据存储保护，不以明文写入 JSON5。
- 配置通过同目录临时文件原子替换，避免异常退出产生残缺 JSON5。

## 支持平台

- Windows 10/11 x64
- macOS ARM64（Apple Silicon）
- 不支持 macOS x64。

WD 和本地 embedding 模型的执行后端由 Rust 负责：Windows 优先使用 DirectML，macOS 优先使用 CoreML，执行后端不可用时回退到 CPU。

## 安装

从 GitHub Releases 下载对应平台的 `.eagleplugin` 文件。2.2.1 版本的文件名为：

- Windows x64：`auto-tagger-eagle-plugin-v2.2.1-win-x64.eagleplugin`
- macOS ARM64：`auto-tagger-eagle-plugin-v2.2.1-mac-arm64.eagleplugin`

下载后打开 Eagle，将 `.eagleplugin` 文件拖入 Eagle 并确认安装。

插件本身不包含大型模型文件。安装后打开设置，选择模型根目录，再从各功能界面下载所需模型或手动放置模型文件。

## 模型目录

模型根目录下使用不同子目录保存不同类型的资源：

```text
模型根目录/
├── wd/
│   └── wd-eva02-large-tagger-v3/
│       ├── model.onnx
│       └── selected_tags.csv
├── llm/
│   └── Qwen3.5-9B-Q4_K_M/
│       ├── Qwen3.5-9B-Q4_K_M.gguf
│       └── mmproj-F16.gguf
├── embedding/
│   └── jina-clip-v2-q8/
│       ├── onnx/model_quantized.onnx
│       ├── tokenizer.json
│       └── 其他模型配置文件
└── llamafile/
    └── llamafile-0.10.4.exe
```

### WD 模型

当前内置下载列表包括：

- `wd-eva02-large-tagger-v3`
- `wd-vit-tagger-v3`
- `wd-vit-large-tagger-v3`
- `wd-v1-4-moat-tagger-v2`
- `cl_tagger`
- `camie-tagger-v2`

WD 模型必须放在 `模型根目录/wd/模型名/` 下，并包含对应格式要求的模型文件和标签元数据。模型是否完整由后端扫描结果决定，文件不完整时不会出现在可用模型列表中。

### LLM 模型

当前内置下载列表包括：

- `Qwen3.5-9B-Q4_K_M`
- `Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-Q4_K_M`
- `Qwen3VL-8B-Instruct-Q4_K_M`
- `Llama-JoyCaption-Alpha-Two-8B-Q6_K`
- `Llama-JoyCaption-Beta-One-8B-Q4_K_M`

每个模型需要放在 `模型根目录/llm/模型名/` 下，并准备 GGUF 主模型和对应的 `mmproj` 文件。llamafile 运行程序放在 `模型根目录/llamafile/` 下，插件可以从设置页面下载。

### Embedding 模型

本地 embedding 模型放在 `模型根目录/embedding/模型名/` 下。当前内置本地模型为 `jina-clip-v2-q8`。

远程 embedding 需要在设置中填写：

- 远程基础网址，只填写域名或服务根路径，插件会自动补全 API 路径。
- API key，使用操作系统凭据存储保护。
- 远程模型名称。
- Gemini 原生 REST 协议的输出维度，范围为 128 到 3072。

不同模型、协议或维度的向量不能互相比较，插件会为它们使用独立索引。

## 使用流程

1. 在设置中选择模型根目录。
2. 在需要的功能页面下载模型，或按目录结构手动放置模型。
3. 在 WD 页面选择图片、模型、标签语言、阈值和覆写方式，然后开始打标。
4. 在 LLM 页面选择模型和操作类型，确认提示词后处理图片。
5. 在语义搜索页面选择 embedding 模型，先建立图片索引，再使用文字或图片搜索。
6. 使用一键处理按需串联打标、注释和三类向量索引。
7. 使用任务中心查看当前任务、进度和失败信息，并在需要时恢复分类备份。

视频读取内容时需要 Eagle 插件中心提供的 ffmpeg。关闭视频内容读取时，视频只按照文件本身参与已有业务流程，不会抽取画面。

## 从源码构建

### 环境要求

- Git
- Bun `1.3.14` 或兼容版本
- Rust `1.94.0`
- Windows 构建需要 Windows x64 工具链。
- macOS ARM64 构建需要在 macOS runner 或 Apple Silicon 环境中使用 Apple 工具链。

### 安装和运行

```powershell
git clone https://github.com/bukkumaaku/auto-tagger-eagle-plugin.git
cd auto-tagger-eagle-plugin
bun install
bun run dev
```

### 构建插件

```powershell
bun run build
```

构建流程会编译 Rust worker、运行 Vue 类型检查、生成 Vite dist，并检查 worker、运行库、manifest 和旧版 JS 文件是否正确。

Rust 测试和 Clippy：

```powershell
cargo test --manifest-path backend/Cargo.toml --workspace
cargo clippy --manifest-path backend/Cargo.toml --workspace --all-targets -- -D warnings
```

GitHub Actions 在推送到 `main` 时构建 Windows x64 和 macOS ARM64；推送与版本一致的 `v2.2.1` 标签时会额外创建 Release。

## 项目结构

```text
backend/
├── crates/protocol       JSON stdin/stdout 协议定义
├── crates/ai-worker      Rust worker 和请求分发
├── crates/wd-tagger      WD/CL/Camie ONNX 打标
├── crates/llamafile      llamafile 生命周期和图像请求
├── crates/*-embedding    本地及远程向量化
└── crates/vector-store   SQLite vec 向量存储和搜索
src/
├── components/           Vue 功能界面
├── services/             worker 客户端和任务中心
└── api/                  Eagle API 与前端业务封装
```

前端只负责界面交互和简单业务编排，模型推理、文件读写、下载、配置、索引和并发控制由 Rust worker 负责。前后端通过 JSON Lines stdin/stdout 通信。

## 问题反馈

请在 GitHub Issues 中提供：

- 操作系统和 Eagle 版本。
- 使用的功能、模型名称和模型目录结构。
- 插件界面中的错误信息或 worker 日志。
- 显卡型号，以及是否使用 DirectML、CoreML 或 CPU。
- 能够稳定复现问题的最小步骤。
