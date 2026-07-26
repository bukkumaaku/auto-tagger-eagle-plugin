import { llamafilePath, modelResourcePath } from "./modelPaths";

async function getModelPath(modelName: string, filename: string) {
    return modelResourcePath("wd", modelName, filename);
}

async function getTypedModelPath(
    modelName: string,
    filename: string,
    type: "wd" | "llm" | "embedding",
) {
    return modelResourcePath(type, modelName, filename);
}

export const wdModelInfo = [
    {
        label: "wd-eva02-large-tagger-v3",
        value: "wd-eva02-large-tagger-v3",
        type: "wd",
        downloadInfo: [
            {
                name: "wd-eva02-large-tagger-v3",
                filename: "model.onnx",
                url: "https://huggingface.co/SmilingWolf/wd-eva02-large-tagger-v3/resolve/main/model.onnx",
                get dest() {
                    return getModelPath(this.name, this.filename);
                },
            },
            {
                name: "wd-eva02-large-tagger-v3",
                filename: "selected_tags.csv",
                url: "https://huggingface.co/SmilingWolf/wd-eva02-large-tagger-v3/resolve/main/selected_tags.csv",
                get dest() {
                    return getModelPath(this.name, this.filename);
                },
            },
        ],
    },
    {
        label: "wd-vit-tagger-v3",
        value: "wd-vit-tagger-v3",
        type: "wd",
        downloadInfo: [
            {
                name: "wd-vit-tagger-v3",
                filename: "model.onnx",
                url: "https://huggingface.co/SmilingWolf/wd-vit-tagger-v3/resolve/main/model.onnx",
                get dest() {
                    return getModelPath(this.name, this.filename);
                },
            },
            {
                name: "wd-vit-tagger-v3",
                filename: "selected_tags.csv",
                url: "https://huggingface.co/SmilingWolf/wd-vit-tagger-v3/resolve/main/selected_tags.csv",
                get dest() {
                    return getModelPath(this.name, this.filename);
                },
            },
        ],
    },
    {
        label: "wd-vit-large-tagger-v3",
        value: "wd-vit-large-tagger-v3",
        type: "wd",
        downloadInfo: [
            {
                name: "wd-vit-large-tagger-v3",
                filename: "model.onnx",
                url: "https://huggingface.co/SmilingWolf/wd-vit-large-tagger-v3/resolve/main/model.onnx",
                get dest() {
                    return getModelPath(this.name, this.filename);
                },
            },
            {
                name: "wd-vit-large-tagger-v3",
                filename: "selected_tags.csv",
                url: "https://huggingface.co/SmilingWolf/wd-vit-large-tagger-v3/resolve/main/selected_tags.csv",
                get dest() {
                    return getModelPath(this.name, this.filename);
                },
            },
        ],
    },
    {
        label: "wd-v1-4-moat-tagger-v2",
        value: "wd-v1-4-moat-tagger-v2",
        type: "wd",
        downloadInfo: [
            {
                name: "wd-v1-4-moat-tagger-v2",
                filename: "selected_tags.csv",
                url: "https://huggingface.co/SmilingWolf/wd-v1-4-moat-tagger-v2/resolve/main/selected_tags.csv",
                get dest() {
                    return getModelPath(this.name, this.filename);
                },
            },
            {
                name: "wd-v1-4-moat-tagger-v2",
                filename: "model.onnx",
                url: "https://huggingface.co/SmilingWolf/wd-v1-4-moat-tagger-v2/resolve/main/model.onnx",
                get dest() {
                    return getModelPath(this.name, this.filename);
                },
            },
        ],
    },
    {
        label: "cl_tagger",
        value: "cl_tagger",
        type: "cl",
        downloadInfo: [
            {
                name: "cl_tagger",
                filename: "model.onnx",
                url: "https://huggingface.co/cella110n/cl_tagger/resolve/main/cl_tagger_1_02/model.onnx",
                get dest() {
                    return getModelPath(this.name, this.filename);
                },
            },
            {
                name: "cl_tagger",
                filename: "tag_mapping.json",
                url: "https://huggingface.co/cella110n/cl_tagger/resolve/main/cl_tagger_1_02/tag_mapping.json",
                get dest() {
                    return getModelPath(this.name, this.filename);
                },
            },
        ],
    },
    {
        label: "camie-tagger-v2",
        value: "camie-tagger-v2",
        type: "ca",
        downloadInfo: [
            {
                name: "camie-tagger-v2",
                filename: "model.onnx",
                url: "https://huggingface.co/Camais03/camie-tagger-v2/resolve/main/camie-tagger-v2.onnx",
                get dest() {
                    return getModelPath(this.name, this.filename);
                },
            },
            {
                name: "camie-tagger-v2",
                filename: "camie-tagger-v2-metadata.json",
                url: "https://huggingface.co/Camais03/camie-tagger-v2/resolve/main/camie-tagger-v2-metadata.json",
                get dest() {
                    return getModelPath(this.name, this.filename);
                },
            },
        ],
    },
];

export const llmModelInfo = [
    {
        label: "llamafile 0.10.4 runner",
        value: "llamafile-0.10.4",
        type: "llm",
        runnerOnly: true,
        downloadInfo: [
            {
                name: "llamafile",
                filename: "llamafile-0.10.4.exe",
                url: "https://github.com/mozilla-ai/llamafile/releases/download/0.10.4/llamafile-0.10.4",
                get dest() {
                    return llamafilePath(this.filename);
                },
            },
        ],
    },
    {
        label: "Qwen3.5-9B-Q4_K_M",
        value: "Qwen3.5-9B-Q4_K_M",
        type: "llm",
        downloadInfo: [
            {
                name: "Qwen3.5-9B-Q4_K_M",
                filename: "Qwen3.5-9B-Q4_K_M.gguf",
                url: "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/resolve/main/Qwen3.5-9B-Q4_K_M.gguf",
                get dest() {
                    return getTypedModelPath(this.name, this.filename, "llm");
                },
            },
            {
                name: "Qwen3.5-9B-Q4_K_M",
                filename: "mmproj-F16.gguf",
                url: "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/resolve/main/mmproj-F16.gguf",
                get dest() {
                    return getTypedModelPath(this.name, this.filename, "llm");
                },
            },
        ],
    },
    {
        label: "Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-Q4_K_M",
        value: "Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-Q4_K_M",
        type: "llm",
        downloadInfo: [
            {
                name: "Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-Q4_K_M",
                filename:
                    "Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-Q4_K_M.gguf",
                url: "https://huggingface.co/HauhauCS/Qwen3.5-9B-Uncensored-HauhauCS-Aggressive/resolve/main/Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-Q4_K_M.gguf",
                get dest() {
                    return getTypedModelPath(this.name, this.filename, "llm");
                },
            },
            {
                name: "Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-Q4_K_M",
                filename:
                    "mmproj-Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-BF16.gguf",
                url: "https://huggingface.co/HauhauCS/Qwen3.5-9B-Uncensored-HauhauCS-Aggressive/resolve/main/mmproj-Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-BF16.gguf",
                get dest() {
                    return getTypedModelPath(this.name, this.filename, "llm");
                },
            },
        ],
    },
    {
        label: "Qwen3VL-8B-Instruct-Q4_K_M",
        value: "Qwen3VL-8B-Instruct-Q4_K_M",
        type: "llm",
        downloadInfo: [
            {
                name: "Qwen3VL-8B-Instruct-Q4_K_M",
                filename: "Qwen3VL-8B-Instruct-Q4_K_M.gguf",
                url: "https://huggingface.co/Qwen/Qwen3-VL-8B-Instruct-GGUF/resolve/main/Qwen3VL-8B-Instruct-Q4_K_M.gguf",
                get dest() {
                    return getTypedModelPath(this.name, this.filename, "llm");
                },
            },
            {
                name: "Qwen3VL-8B-Instruct-Q4_K_M",
                filename: "mmproj-Qwen3VL-8B-Instruct-F16.gguf",
                url: "https://huggingface.co/Qwen/Qwen3-VL-8B-Instruct-GGUF/resolve/main/mmproj-Qwen3VL-8B-Instruct-F16.gguf",
                get dest() {
                    return getTypedModelPath(this.name, this.filename, "llm");
                },
            },
        ],
    },
    {
        label: "Llama-JoyCaption-Alpha-Two-8B-Q6_K",
        value: "Llama-JoyCaption-Alpha-Two-8B-Q6_K",
        type: "llm",
        downloadInfo: [
            {
                name: "Llama-JoyCaption-Alpha-Two-8B-Q6_K",
                filename: "llama-joycaption-alpha-two-llava-Q6_K.gguf",
                url: "https://huggingface.co/Jobaar/Llama-JoyCaption-Alpha-Two-GGUF/resolve/main/llama-joycaption-alpha-two-llava-Q6_K.gguf",
                get dest() {
                    return getTypedModelPath(this.name, this.filename, "llm");
                },
            },
            {
                name: "Llama-JoyCaption-Alpha-Two-8B-Q6_K",
                filename:
                    "llama-joycaption-alpha-two-llava-mmproj-model-f16.gguf",
                url: "https://huggingface.co/Jobaar/Llama-JoyCaption-Alpha-Two-GGUF/resolve/main/llama-joycaption-alpha-two-llava-mmproj-model-f16.gguf",
                get dest() {
                    return getTypedModelPath(this.name, this.filename, "llm");
                },
            },
        ],
    },
];

export const embeddingModelInfo = [
    {
        label: "jina-clip-v2-q8",
        value: "jina-clip-v2-q8",
        type: "embedding",
        downloadInfo: [
            {
                name: "jina-clip-v2-q8",
                filename: "config.json",
                url: "https://huggingface.co/jinaai/jina-clip-v2/resolve/main/config.json",
                get dest() {
                    return getTypedModelPath(this.name, this.filename, "embedding");
                },
            },
            {
                name: "jina-clip-v2-q8",
                filename: "preprocessor_config.json",
                url: "https://huggingface.co/jinaai/jina-clip-v2/resolve/main/preprocessor_config.json",
                get dest() {
                    return getTypedModelPath(this.name, this.filename, "embedding");
                },
            },
            {
                name: "jina-clip-v2-q8",
                filename: "tokenizer.json",
                url: "https://huggingface.co/jinaai/jina-clip-v2/resolve/main/tokenizer.json",
                get dest() {
                    return getTypedModelPath(this.name, this.filename, "embedding");
                },
            },
            {
                name: "jina-clip-v2-q8",
                filename: "tokenizer_config.json",
                url: "https://huggingface.co/jinaai/jina-clip-v2/resolve/main/tokenizer_config.json",
                get dest() {
                    return getTypedModelPath(this.name, this.filename, "embedding");
                },
            },
            {
                name: "jina-clip-v2-q8",
                filename: "onnx/model_quantized.onnx",
                url: "https://huggingface.co/jinaai/jina-clip-v2/resolve/main/onnx/model_quantized.onnx",
                get dest() {
                    return getTypedModelPath(this.name, this.filename, "embedding");
                },
            },
        ],
    },
];
