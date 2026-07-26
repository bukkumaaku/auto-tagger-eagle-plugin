# Eagle Auto Tagger

English | [简体中文](./README.md)

Eagle Auto Tagger is an Eagle plugin with a Rust backend and Vue interface. It provides image and video tagging, vision-language tag and annotation generation, one-click workflows, and combined semantic search over images, tags, and annotations.

Current version: `2.2.1`

## Features

### WD image tagging

- Supports PNG, JPG, JPEG, WEBP, and BMP images. GIF files are skipped.
- Supports batch image tagging.
- Supports Chinese, English, or mixed labels with a configurable separator.
- Supports skip, overwrite, and merge modes for existing tags.
- Configurable confidence threshold and batch size.
- If an individual image cannot be read, it is skipped and reported in the task result.
- Videos can either skip content reading or use ffmpeg to extract frames at 1%, 20%, 40%, 60%, 80%, and 99% of the video duration before tagging.
- Supports WD, CL-Tagger, and Camie model formats. The backend scans the actual model files before showing a model as available.

### LLM image understanding

- Supports local llamafile + GGUF vision-language models and remote OpenAI-compatible or native Gemini REST vision models.
- Stores multiple remote LLM profiles and supports configurable concurrency for remote batches.
- Can generate tags or write an annotation to Eagle.
- Tagging and annotation prompts can be edited in the interface and are saved separately.
- Provides controls to restore the default prompts.
- Supports skip, overwrite, and merge modes for existing tags or annotations.
- Built-in local models include Qwen3.5-9B, Qwen3-VL 8B, and Llama JoyCaption Alpha Two 8B. Each model requires its GGUF main model and mmproj file.
- llamafile stdout and stderr are written by date to `model-root/logs/YYYY-MM-DD.log`.
- Local LLM startup can auto-detect CUDA, Metal, and Vulkan, report unavailable backends clearly, and optionally fall back to Vulkan or CPU.

### Semantic image search

- Builds a vector index for Eagle images and skips images that are already indexed.
- Supports pause, resume, configurable batch size, and index progress reporting.
- Supports text-to-image and image-to-image search.
- Sorts results by similarity and displays them in a lazy-loaded masonry layout.
- Click a result to preview it and double-click to open it in Eagle.
- Supports a local Jina CLIP v2 model.
- Supports OpenAI Compatible and native Gemini REST remote embedding services.
- Stores multiple remote embedding profiles.
- Indexes images, tags, and annotations separately and combines them with configurable weights.
- Supports positive and negative text conditions combined with image similarity.
- Different models, protocols, and vector dimensions use separate SQLite vector tables so incompatible vector spaces are never mixed.
- Provides index health checks, rebuild recovery, stale-item cleanup, and a list of failed items.

### One-click workflows and backups

- Runs WD tagging, LLM tagging, LLM annotation, and image/tag/annotation vectorization as one configurable workflow.
- Supports pausing, resuming, and cancelling long tasks with stage progress and failure reporting.
- Creates categorized tag or annotation backups before batch writes and allows restoring a selected backup.

### Tasks and configuration

- A unified task center shows tagging, indexing, search, and download tasks.
- Model downloads report progress and surface errors.
- On startup, the plugin checks GitHub Releases and can open the Release download page when a newer version is available.
- Configuration is read and written by the Rust backend. The default file is `.eagle-auto-tagger/config.json5` under the current user's home directory.
- Newly added configuration keys are written to JSON5 with their default values.
- Remote embedding and LLM profile API keys are protected by the operating system credential store and are not written to JSON5 in plain text.
- Configuration writes use an atomic same-directory replacement so an interrupted write cannot leave partial JSON5.

## Supported platforms

- Windows 10/11 x64
- macOS ARM64 (Apple Silicon)
- macOS x64 is not supported.

WD and local embedding inference are handled by Rust: Windows tries DirectML first, macOS tries CoreML first, and CPU is used when the accelerated provider is unavailable.

## Installation

Download the `.eagleplugin` file for your platform from GitHub Releases. The 2.2.1 files are:

- Windows x64: `auto-tagger-eagle-plugin-v2.2.1-win-x64.eagleplugin`
- macOS ARM64: `auto-tagger-eagle-plugin-v2.2.1-mac-arm64.eagleplugin`

Open Eagle, drag the `.eagleplugin` file into the application, and confirm the installation.

Large model files are not included in the plugin package. After installation, open Settings, select a model root, and download the required models from the plugin or place them manually in the expected directories.

## Model directories

The model root uses separate subdirectories for each resource type:

```text
model-root/
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
│       └── other model configuration files
└── llamafile/
    └── llamafile-0.10.4.exe
```

### WD models

The built-in download list currently includes:

- `wd-eva02-large-tagger-v3`
- `wd-vit-tagger-v3`
- `wd-vit-large-tagger-v3`
- `wd-v1-4-moat-tagger-v2`
- `cl_tagger`
- `camie-tagger-v2`

WD models must be placed under `model-root/wd/model-name/` with the model files and tag metadata required by that format. The backend decides whether a model is complete; incomplete models are not shown as available.

### LLM models

The built-in download list currently includes:

- `Qwen3.5-9B-Q4_K_M`
- `Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-Q4_K_M`
- `Qwen3VL-8B-Instruct-Q4_K_M`
- `Llama-JoyCaption-Alpha-Two-8B-Q6_K`

Each model belongs under `model-root/llm/model-name/` and requires a GGUF main model plus its corresponding `mmproj` file. The llamafile runner belongs under `model-root/llamafile/` and can be downloaded from Settings.

### Embedding models

Local embedding models belong under `model-root/embedding/model-name/`. The current built-in local model is `jina-clip-v2-q8`.

For remote embeddings, configure:

- A remote base URL. Enter only the service base URL; the plugin completes the API path.
- An API key, protected by the operating system credential store.
- The exact remote model name.
- The output dimension for the native Gemini REST protocol, from 128 to 3072.

Vectors from different models, protocols, or dimensions cannot be compared. The plugin keeps them in separate indexes.

## Usage

1. Select a model root in Settings.
2. Download models from the relevant page or place them in the directory structure above.
3. In WD Tagger, select images, a model, label language, threshold, and overwrite mode, then start tagging.
4. In LLM Image Understanding, select a model and operation, review the prompt, and process the image.
5. In Semantic Search, select an embedding model, build an image index, and then search by text or image.
6. Use One-click Workflow to combine tagging, annotation, and all three vector indexes as needed.
7. Use the task center to monitor progress and failures, and restore categorized backups when required.

Reading video content requires ffmpeg provided by the Eagle plugin center. When video content reading is disabled, the plugin does not extract frames from videos.

## Build from source

### Requirements

- Git
- Bun `1.3.14` or a compatible version
- Rust `1.94.0`
- Windows x64 builds require the Windows x64 Rust toolchain.
- macOS ARM64 builds require the Apple toolchain on a macOS runner or Apple Silicon environment.

### Install and run

```powershell
git clone https://github.com/bukkumaaku/auto-tagger-eagle-plugin.git
cd auto-tagger-eagle-plugin
bun install
bun run dev
```

### Build the plugin

```powershell
bun run build
```

The build compiles the Rust worker, runs Vue type checking, creates the Vite dist directory, and verifies the worker, runtime files, manifest, and absence of the legacy JavaScript runtime.

Rust tests and Clippy:

```powershell
cargo test --manifest-path backend/Cargo.toml --workspace
cargo clippy --manifest-path backend/Cargo.toml --workspace --all-targets -- -D warnings
```

GitHub Actions builds Windows x64 and macOS ARM64 when `main` is pushed. Pushing a version-matching `v2.2.1` tag also creates a Release.

## Project structure

```text
backend/
├── crates/protocol       JSON Lines stdin/stdout protocol definitions
├── crates/ai-worker      Rust worker and request dispatch
├── crates/wd-tagger      WD/CL/Camie ONNX tagging
├── crates/llamafile      llamafile lifecycle and image requests
├── crates/*-embedding    Local and remote embeddings
└── crates/vector-store   SQLite vec storage and search
src/
├── components/           Vue feature interfaces
├── services/             Worker client and task center
└── api/                  Eagle API and frontend orchestration
```

The frontend handles interface interaction and light orchestration. The Rust worker handles inference, file I/O, downloads, configuration, indexing, and concurrency. The frontend and backend communicate through JSON Lines over stdin/stdout.

## Issues and feedback

When opening an issue, include:

- Operating system and Eagle version.
- The feature, model name, and model directory structure involved.
- The error message or worker log from the plugin.
- GPU model and whether DirectML, CoreML, or CPU was used.
- The smallest set of steps that reproduces the problem.
