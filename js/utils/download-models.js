const https = require("https");
//const fs = require("fs-extra");
const path = require("path");

const BASE_URL = {
    mirror: "https://hf-mirror.com",
    direct: "https://huggingface.co",
};
const MODEL_DIR = __dirname + "/models";
const DOWNLOAD_LIST = {
    wd: {
        model: "model.onnx",
        tags: "selected_tags.csv",
        author: "SmilingWolf",
        extra: "",
    },
    cl: {
        model: "model.onnx",
        tags: "tag_mapping.json",
        author: "cella110n",
        extra: "cl_tagger_1_02/",
    },
    ca: {
        model: "camie-tagger-v2.onnx",
        tags: "camie-tagger-v2-metadata.json",
        author: "Camais03",
        extra: "",
    },
};
let downloadUrl;
async function downloadFile(modelName, downloadWebsite, type, onProgress) {
    downloadUrl =
        downloadWebsite === "mirror" ? BASE_URL.mirror : BASE_URL.direct;
    console.log(`Preparing to download ${MODEL_DIR} from ${modelName}`);
    await fs.ensureDir(path.join(MODEL_DIR, modelName));
    const modelFileName = DOWNLOAD_LIST[type].model;
    const tagsFileName = DOWNLOAD_LIST[type].tags;
    const authorName = DOWNLOAD_LIST[type].author;
    const extraDir = DOWNLOAD_LIST[type].extra;
    const modelUrl = `${downloadUrl}/${authorName}/${modelName}/resolve/main/${extraDir}${modelFileName}`;
    const tagsUrl = `${downloadUrl}/${authorName}/${modelName}/resolve/main/${extraDir}${tagsFileName}`;
    console.log(path.join(MODEL_DIR, modelName, modelFileName));
    if (
        (await fs.pathExists(path.join(MODEL_DIR, modelName, modelFileName))) &&
        (await fs.pathExists(path.join(MODEL_DIR, modelName, tagsFileName)))
    ) {
        console.log("Model and tags already exist. Skipping download.");
        onProgress(100, "0 MB/s", "tags");
        onProgress(100, "0 MB/s", "model");
        return;
    } else {
        await fs.remove(path.join(MODEL_DIR, modelName));
        await fs.ensureDir(path.join(MODEL_DIR, modelName));
        console.log(`Downloading ${modelName} model and tags...`);
    }
    console.log(`Downloading ${modelUrl}...`);
    await downloadProcess(tagsUrl, modelName, tagsFileName, onProgress, "tags");
    await downloadProcess(
        modelUrl,
        modelName,
        modelFileName,
        onProgress,
        "model"
    );
}

async function downloadProcess(url, modelName, fileName, onProgress, type) {
    return new Promise((resolve, reject) => {
        // 定义一个内部函数，用来处理可能的重定向
        const makeRequest = (currentUrl) => {
            console.log(`Downloading ${currentUrl}`);
            https
                .get(currentUrl, (response) => {
                    // === 1. 处理重定向 (301, 302, 307, 308) ===
                    if (
                        response.statusCode >= 300 &&
                        response.statusCode < 400 &&
                        response.headers.location
                    ) {
                        // 递归调用，去请求新的地址
                        currentUrl = response.headers.location;
                        if (
                            !currentUrl.startsWith("http://") &&
                            !currentUrl.startsWith("https://")
                        ) {
                            currentUrl = downloadUrl + currentUrl;
                        }
                        makeRequest(currentUrl);
                        return;
                    }

                    // === 2. 处理真正的错误 ===
                    if (response.statusCode !== 200) {
                        reject(
                            new Error(
                                `Failed to download: ${response.statusCode}`
                            )
                        );
                        return;
                    }

                    // === 3. 开始正常下载 (状态码 200) ===
                    const totalLength = parseInt(
                        response.headers["content-length"],
                        10
                    );
                    let downloadedLength = 0;
                    const startTime = Date.now();

                    const writer = fs.createWriteStream(
                        path.join(MODEL_DIR, modelName, fileName)
                    );

                    response.on("data", (chunk) => {
                        downloadedLength += chunk.length;

                        const elapsedTime = (Date.now() - startTime) / 1000;
                        const safeTime = elapsedTime > 0 ? elapsedTime : 1;

                        const speed = (
                            downloadedLength /
                            1024 /
                            1024 /
                            safeTime
                        ).toFixed(2);
                        const progress = totalLength
                            ? ((downloadedLength / totalLength) * 100).toFixed(
                                  2
                              )
                            : 0;

                        onProgress(progress, speed + " MB/s", type);
                    });

                    response.pipe(writer);
                    onProgress(100, "0 MB/s", type);
                    writer.on("finish", () => {
                        writer.close(resolve);
                    });

                    writer.on("error", (err) => {
                        // 下载出错时删除可能损坏的文件
                        fs.unlink(
                            path.join(MODEL_DIR, modelName, fileName),
                            () => {}
                        );
                        reject(err);
                    });
                })
                .on("error", (err) => {
                    // 处理网络层面的错误（如断网、DNS解析失败）
                    reject(err);
                });
        };

        // 启动第一次请求
        makeRequest(url);
    });
}
