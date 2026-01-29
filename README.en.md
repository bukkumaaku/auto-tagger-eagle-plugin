# Auto Tagger Plugin for Eagle

English | [简体中文](./README.md)

## Eagle Auto Tagger Plugin

Automatically generate descriptive tags for files with thumbnails in the Eagle repository. No need to install Python or Node.js environment, just download and use. Supports GPU acceleration (DirectML/WebGPU) and automatic CPU fallback.

### ✨ Special function

- Zero dependency installation: No need to configure Node.js, Python or any development environment, just download and unzip it and import it into Eagle.
- Intelligent recognition: automatically analyze image/video content and generate accurate tags.
- Hardware acceleration:
- Prioritize GPU acceleration (supports DirectML / WebGPU, no CUDA required).
- Automatically switches to CPU mode when there is no graphics card or insufficient video memory.
- Multi-model support: Compatible with mainstream models such as WDv2, Vitv3, CL-Tagger, etc.

### 📥 Installation guide

1. Download the plug-in
   Please go to the Releases page to download the latest version of the plug-in format:

- Windows users: Download auto-tagger-eagle-plugin-win-x64.eagleplugin
- macOS users: Download auto-tagger-eagle-plugin-mac-arm64.eagleplugin (only supports Apple Silicon M1/M2/M3...)

2. Import Eagle

- Downloaded .eagleplugin file.
- Open the Eagle software.
- Drag the .eagleplugin file into eagle and click install
    > System requirements:
    >
    > - Windows 10/11 x64
    > - macOS 14+ (Apple Silicon)
    > - Eagle App v4.0 and above

### 🧠 Model configuration

This plug-in needs to be used with model files. In order to reduce the size, the plug-in package does not include pre-trained models, please download on demand or download from the plugin interface.

1. Get the model
   Please visit [HuggingFace - SmilingWolf](https://huggingface.co/SmilingWolf) to download the following recommended models (just download the two files model.onnx and selected_tags.csv (except CL-Tagger)):

- Recommended: wd-v1-4-moat-tagger-v2 (best overall effect)
- High precision: wd-vit-tagger-v3
- Others: CL-Tagger (requires json mapping table)

2. Place the model

- Place the downloaded file into the models folder in the plugin directory. The recommended structure is as follows:

```
   auto-tagger-eagle-plugin/
   └── models/
   ├── wd-v1-4-moat-tagger-v2/ <-- The folder name is the model name
   │ ├── model.onnx
   │ └── selected_tags.csv
   └── wd-vit-tagger-v3/
   ├── model.onnx
   └── selected_tags.csv
```

Note: Model.onnx and tag_mapping.json are placed under the cL-tagger model folder.

### 🚀 Instructions for use

- Select one or more pictures in Eagle.
- Right-click or the plug-in icon and select "Auto Tagger".
- Select the model (such as wd-v1-4-moat-tagger-v2) in the pop-up panel.
- Click Start and wait for the label to be generated.

⚠️ Notes
| Model name | Notes |
|---|---|
| wd-v1-4-moat-tagger-v2 | ⚠️ Batch Size can only be set to 1, otherwise an error may be reported. |
| wd-vit-tagger-v3 | The video memory usage is slightly higher, the speed is slightly slower, but the accuracy is higher. |
| cl_tagger | It is recommended that the confidence threshold be set greater than 0.55, otherwise too many interference tags will be generated. |

### 🛠️ Developer Guide (Build from source)

If you want to modify the code or compile the plug-in yourself, please refer to the following steps (ordinary users please skip):
Environmental preparation

- Node.js 20+
- Git

#### 1. Clone repository

```
git clone https://github.com/bukkumaaku/auto-tagger-eagle-plugin.git
cd auto-tagger-eagle-plugin
```

#### 2. Install dependencies

```
npm install
```

### 🤝 Contribution and Feedback

Have a problem or have a new feature suggestion? Welcome to submit Issues.
When submitting an issue, please provide:

- Operating system version
- Error log screenshot (press F12 in the plug-in interface to view the console that appears)
- Graphics card model
