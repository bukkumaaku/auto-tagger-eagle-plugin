# Auto Tagger Plugin for Eagle

## Eagle 自动打标插件
为 Eagle 资源库中有缩略图的文件自动生成描述性标签。无需安装 Python 或 Node.js 环境，下载即用。支持 GPU 加速（DirectML/WebGPU）与 CPU 自动回退。

### ✨ 功能特性

-   零依赖安装：无需配置 Node.js、Python 或任何开发环境，下载解压即可导入 Eagle。
-   智能识别：自动分析图片/视频内容并生成精准标签。
-   硬件加速：
    -   优先使用 GPU 加速（支持 DirectML / WebGPU，无需 CUDA）。
    -   无显卡或显存不足时自动切换至 CPU 模式。
-   多模型支持：兼容 WDv2、Vitv3、CL-Tagger 等主流模型。

### 📥 安装指南

1. 下载插件
   请前往 Releases 页面 下载最新版本的压缩包：

-   Windows 用户: 下载 auto-tagger-eagle-plugin-win-x64.zip
-   macOS 用户: 下载 auto-tagger-eagle-plugin-mac-arm64.zip (仅支持 Apple Silicon M1/M2/M3...)

2. 导入 Eagle

-   解压下载的 .zip 文件。
-   打开 Eagle 软件。
-   点击左侧边栏的 “插件中心” (Plugin) > “管理插件”。
-   点击 “导入插件”，选择刚才解压出来的文件夹即可。
    > 系统要求：
    >
    > -   Windows 10/11 x64
    > -   macOS 14+ (Apple Silicon)
    > -   Eagle App v4.0 及以上版本

### 🧠 模型配置
本插件需配合模型文件使用。为了减小体积，插件包内不包含预训练模型，请按需下载。

1. 获取模型
   请访问 HuggingFace - SmilingWolf 下载以下推荐模型（下载 model.onnx 和 selected_tags.csv 两个文件即可(CL-Tagger 除外)）：

-   推荐：wd-v1-4-moat-tagger-v2 (综合效果最好)
-   高精度：wd-vit-tagger-v3
-   其他：CL-Tagger (需配合 json 映射表)

2. 放置模型
-   将下载的文件放入插件目录下的 models 文件夹中。推荐结构如下：
```
   auto-tagger-eagle-plugin/
   └── models/
   ├── wd-v1-4-moat-tagger-v2/ <-- 文件夹名即为模型名
   │ ├── model.onnx
   │ └── selected_tags.csv
   └── wd-vit-tagger-v3/
   ├── model.onnx
   └── selected_tags.csv
   ```

注：cL-tagger 模型文件夹下放置的是 model.onnx 和 tag_mapping.json

### 🚀 使用说明

-   在 Eagle 中选中一张或多张图片。
-   点击右键或插件图标，选择 "Auto Tagger"。
-   在弹出的面板中选择模型（如 wd-v1-4-moat-tagger-v2）。
-   点击开始，等待标签生成完毕。

⚠️ 注意事项
| 模型名称 | 注意事项 |
|---|---|
| wd-v1-4-moat-tagger-v2 | ⚠️ 批次大小 (Batch Size) 只能设置为 1，否则可能报错。 |
| wd-vit-tagger-v3 | 显存占用稍高，速度稍慢，但精度更高。 |
| cl-tagger | 建议置信度阈值设置大于 0.55，否则会产生过多干扰标签。 |

### 🛠️ 开发者指南 (Build from source)

如果你想自行修改代码或编译插件，请参考以下步骤（普通用户请跳过）：
环境准备

-   Node.js 20+
-   Git

#### 1. 克隆仓库

```
git clone https://github.com/bukkumaaku/auto-tagger-eagle-plugin.git
cd auto-tagger-eagle-plugin
```

#### 2. 安装依赖
```
npm install
```
### 🤝 贡献与反馈
遇到问题或有新功能建议？欢迎提交 Issues。
提交 Issue 时请提供：

-   操作系统版本
-   错误日志截图 (在插件界面按 F12 出现的控制台查看)
-   显卡型号
