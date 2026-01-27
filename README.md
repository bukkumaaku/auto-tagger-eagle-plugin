Auto Tagger Plugin for Eagle

Eagle Auto Tagger Plugin

Automatically generates descriptive tags for files with thumbnails in your Eagle library.
No Python or Node.js environment required — download and use instantly.
Supports GPU acceleration (DirectML / WebGPU) with automatic CPU fallback.

✨ Features

Zero-dependency installation
No need to install or configure Node.js, Python, or any development environment. Just download, extract, and import into Eagle.

Intelligent content recognition
Automatically analyzes image/video content and generates accurate, descriptive tags.

Hardware acceleration

Prioritizes GPU acceleration (DirectML / WebGPU, no CUDA required)

Automatically falls back to CPU when no GPU is available or VRAM is insufficient

Multiple model support
Compatible with WDv2, ViT v3, CL-Tagger, and other mainstream tagging models.

📥 Installation Guide
Download the plugin

Go to the Releases page and download the latest version:

Windows:
auto-tagger-eagle-plugin-win-x64.eagleplugin

macOS (Apple Silicon only):
auto-tagger-eagle-plugin-mac-arm64.eagleplugin
(Supports M1 / M2 / M3 and later)

Import into Eagle

Extract the downloaded .eagleplugin file.

Open Eagle.

Drag the .eagleplugin file into Eagle and click Install.

System Requirements

Windows 10 / 11 (x64)

macOS 14+ (Apple Silicon only)

Eagle App v4.0 or later

🧠 Model Configuration

This plugin requires external model files.
To keep the plugin lightweight, pretrained models are not included and must be downloaded separately.

Download models

Visit HuggingFace – SmilingWolf and download the following recommended models
(download model.onnx and selected_tags.csv, except for CL-Tagger):

Recommended (best overall): wd-v1-4-moat-tagger-v2

High accuracy: wd-vit-tagger-v3

Other: CL-Tagger (requires an additional JSON mapping file)

Place model files

Put the downloaded files into the plugin’s models directory.
Recommended structure:

auto-tagger-eagle-plugin/
└── models/
    ├── wd-v1-4-moat-tagger-v2/
    │   ├── model.onnx
    │   └── selected_tags.csv
    └── wd-vit-tagger-v3/
        ├── model.onnx
        └── selected_tags.csv


Note:
For CL-Tagger, the model folder should contain:

model.onnx

tag_mapping.json

🚀 How to Use

Select one or more images in Eagle.

Right-click or click the plugin icon and choose “Auto Tagger”.

Select a model (e.g. wd-v1-4-moat-tagger-v2) in the popup panel.

Click Start and wait for tag generation to complete.

⚠️ Notes & Recommendations
Model Name	Notes
wd-v1-4-moat-tagger-v2	⚠️ Batch Size must be set to 1, otherwise errors may occur
wd-vit-tagger-v3	Higher VRAM usage, slower speed, but better accuracy
cl_tagger	Recommended confidence threshold > 0.55 to avoid excessive noisy tags
🛠️ Developer Guide (Build from Source)

If you want to modify the code or build the plugin yourself
(regular users can skip this section)

Requirements

Node.js 20+

Git

1. Clone the repository
git clone https://github.com/bukkumaaku/auto-tagger-eagle-plugin.git
cd auto-tagger-eagle-plugin

2. Install dependencies
npm install

🤝 Contribution & Feedback

Found a bug or have a feature request?
Feel free to submit an Issue. Please include:

Operating system version

Error log screenshot
(press F12 in the plugin interface to open the console)

GPU model
