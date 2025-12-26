const ort = require("onnxruntime-node");
const csv = require("csv-parser");
const fs = require("fs-extra");
const sharp = require("sharp");
const path = require("path");
const ffmpeg = require("fluent-ffmpeg");
const video_ext = ["mp4", "avi", "mov", "mkv", "flv", "wmv"];

class wdModel {
    constructor(filePath) {
        this.sizeSetting = 448;
        this._shape = [1, this.sizeSetting, this.sizeSetting, 3];
        this.modelFolderPath = path.join(
            __dirname,
            "..",
            "..",
            "models",
            filePath
        );
        this.modelPath = path.join(this.modelFolderPath, "model.onnx");
        this.tagsPath = path.join(this.modelFolderPath, "selected_tags.csv");
    }
    async checkTagsFileExists() {
        if (fs.existsSync(this.tagsPath, fs.constants.R_OK)) {
            return await this.readTagsFile();
        }
    }
    async readTagsFile() {
        const result = [];
        const csvData = await new Promise((resolve, reject) => {
            fs.createReadStream(this.tagsPath)
                .pipe(csv())
                .on("data", (data) => {
                    result.push(data);
                })
                .on("end", () => {
                    resolve(result);
                })
                .on("error", (error) => {
                    reject(error);
                });
        });
        return csvData;
    }
    async preprocessImage(imagePath) {
        let image = sharp(imagePath);
        image = image.flatten();
        image = image.resize({
            width: this.sizeSetting,
            height: this.sizeSetting,
            fit: "contain",
            background: { r: 0, g: 0, b: 0 },
        });
        const rawPixelData = await image.raw().toBuffer();
        const channels = 3;
        const inputData = new Float32Array(
            this.sizeSetting * this.sizeSetting * channels
        );
        return this.extraPreProcess(inputData, rawPixelData);
    }
    extraPreProcess(inputData, rawPixelData) {
        for (let i = 0; i < rawPixelData.length; i += 3) {
            for (let channel = 0; channel < 3; channel++) {
                inputData[i + 2 - channel] = rawPixelData[i + channel] || 0;
            }
        }
        return inputData;
    }
    getClippedValue(cpuData) {
        return cpuData;
    }
    get arr() {
        return [...this._shape];
    }
}

class clModel extends wdModel {
    constructor(filePath) {
        super(filePath);
        this._shape = [1, 3, this.sizeSetting, this.sizeSetting];
        this.tagsPath = path.join(this.modelFolderPath, "tag_mapping.json");
    }
    async readTagsFile() {
        const result = [];
        const config = require(this.tagsPath);
        const len = Object.keys(config).length;
        for (let i = 0; i < len; i++) {
            result.push({ name: config[i].tag });
        }
        return result;
    }
    extraPreProcess(inputData, rawPixelData) {
        const mean = 0.5;
        const std = 0.5;
        const channels = 3;
        for (let y = 0; y < this.sizeSetting; y++) {
            for (let x = 0; x < this.sizeSetting; x++) {
                const i = (y * this.sizeSetting + x) * channels;
                const r = rawPixelData[i];
                const g = rawPixelData[i + 1];
                const b = rawPixelData[i + 2];
                const normalized_b = (b / 255.0 - mean) / std;
                const normalized_g = (g / 255.0 - mean) / std;
                const normalized_r = (r / 255.0 - mean) / std;
                inputData[y * this.sizeSetting + x] = normalized_b;
                inputData[
                    this.sizeSetting * this.sizeSetting +
                        y * this.sizeSetting +
                        x
                ] = normalized_g;
                inputData[
                    2 * this.sizeSetting * this.sizeSetting +
                        y * this.sizeSetting +
                        x
                ] = normalized_r;
            }
        }
        return inputData;
    }
    getClippedValue(cpuData) {
        for (let i = 0; i < cpuData.length; i++) {
            let value = cpuData[i];
            if (isNaN(value)) {
                value = 0.0;
            } else if (value === Infinity) {
                value = 1.0;
            } else if (value === -Infinity) {
                value = 0.0;
            }
            const clippedValue = Math.max(-30, Math.min(value, 30));
            const prob = 1 / (1 + Math.exp(-clippedValue));
            cpuData[i] = prob;
        }
        return cpuData;
    }
}

class caModel extends clModel {
    constructor(filePath) {
        super(filePath);
        this.sizeSetting = 512;
        this._shape = [1, 3, this.sizeSetting, this.sizeSetting];
        this.tagsPath = path.join(
            this.modelFolderPath,
            "camie-tagger-v2-metadata.json"
        );
        this.modelPath = path.join(
            this.modelFolderPath,
            "camie-tagger-v2.onnx"
        );
    }

    async readTagsFile() {
        const config = require(this.tagsPath);
        const mapping = config.dataset_info.tag_mapping.idx_to_tag;
        return mapping;
    }

    extraPreProcess(inputData, rawPixelData) {
        const mean = [0.485, 0.456, 0.406];
        const std = [0.229, 0.224, 0.225];
        const imageSize = this.sizeSetting * this.sizeSetting;
        for (let i = 0; i < imageSize; i++) {
            const offset = i * 3;
            const r = rawPixelData[offset];
            const g = rawPixelData[offset + 1];
            const b = rawPixelData[offset + 2];
            const normalized_r = (r / 255.0 - mean[0]) / std[0];
            const normalized_g = (g / 255.0 - mean[1]) / std[1];
            const normalized_b = (b / 255.0 - mean[2]) / std[2];
            inputData[i] = normalized_r;
            inputData[imageSize + i] = normalized_g;
            inputData[2 * imageSize + i] = normalized_b;
        }
        return inputData;
    }
}

async function createSession(modelPath) {
    const executionProviders = [];
    if (eagle.os.type() === "Windows_NT") {
        executionProviders.push({
            name: "dml",
        });
    } else {
        executionProviders.push({
            name: "webgpu",
        });
        executionProviders.push({
            name: "coreml",
        });
    }
    executionProviders.push({
        name: "cpu",
    });
    return ort.InferenceSession.create(modelPath, {
        executionProviders: executionProviders,
        graphOptimizationLevel: "all",
        executionMode: "parallel",
        enableCpuMemArena: true,
        enableMemPattern: true,
    });
}

async function runInference(imagePath, modelInfo) {
    const results = [];
    for (let i = 0; i < imagePath.length; i++) {
        const floatData = await modelInfo.preprocessImage(imagePath[i]);
        if (floatData.length === 0) {
            results.push([]);
            continue;
        }
        const arr = modelInfo.arr;
        const tensor = new ort.Tensor("float32", floatData, arr);
        results.push(tensor);
    }
    return results;
}

function getVideoDuration(videoPath) {
    return new Promise((resolve, reject) => {
        ffmpeg.ffprobe(videoPath, (err, metadata) => {
            if (err) {
                return reject(err);
            }
            const duration = metadata.format.duration;
            if (duration) {
                resolve(duration);
            } else {
                reject(new Error("无法获取视频时长"));
            }
        });
    });
}

function takeScreenshots(videoPath, outputFolder, timestamps) {
    return new Promise((resolve, reject) => {
        const outputFilenames = [];
        const command = ffmpeg(videoPath)
            .on("filenames", function (filenames) {
                console.log("成功生成截图: " + filenames.join(", "));
                filenames.forEach((name) =>
                    outputFilenames.push(path.join(outputFolder, name))
                );
            })
            .on("end", function () {
                resolve(outputFilenames);
            })
            .on("error", function (err) {
                console.error("截图过程中发生错误: " + err.message);
                reject(err);
            })
            .screenshots({
                timemarks: timestamps,
                filename: "video.png",
                folder: outputFolder,
            });
    });
}

async function getTag(
    imagePath,
    session,
    tagSet,
    config,
    imageItemsBatchs,
    modelInfo,
    isRecursion = false
) {
    const results = await runInference(imagePath, modelInfo);
    let resultsLength = results.length;
    //if (results[0]?.data === undefined) return [];
    const occupy = [];
    for (let i = 0; i < resultsLength; i++) {
        if (
            results[i] === undefined ||
            (Array.isArray(results[i]) && results[i].length === 0)
        ) {
            imagePath.splice(i, 1);
            results.splice(i, 1);
            i--;
            resultsLength--;
            occupy.push(0);
            continue;
        }
        occupy.push(1);
    }
    if (imagePath.length === 0) return [[]];
    const concatenatedData = new Float32Array(
        imagePath.length * 3 * modelInfo.sizeSetting * modelInfo.sizeSetting
    );
    let offset = 0;
    for (const tensor of results) {
        concatenatedData.set(tensor.data, offset);
        offset += tensor.data.length;
    }
    // 运行模型
    const arr = modelInfo.arr;
    arr[0] = imagePath.length;
    const batchTensor = new ort.Tensor("float32", concatenatedData, arr);
    const feeds = {};
    if (session.inputNames[0]) {
        feeds[session.inputNames[0]] = batchTensor;
    }
    const result = await session.run(feeds);
    if (result === undefined) return [[]];
    console.log(result);
    let cpuData =
        result?.output?.data ||
        result?.output?.cpuData ||
        result?.predictions_sigmoid?.cpuData ||
        result?.output0?.cpuData ||
        result?.refined_predictions.cpuData;
    cpuData = modelInfo.getClippedValue(cpuData);
    const perBatchSize = cpuData.length / imagePath.length;
    let ans = [];
    for (let i = 0; i < imagePath.length; i++) {
        let tag = [];
        for (let j = 0; j < perBatchSize; j++) {
            if (cpuData[i * perBatchSize + j] > config.threshold) {
                tag.push(tagSet[j].name || tagSet[j]);
            }
        }
        if (
            video_ext.includes(imageItemsBatchs[i]?.ext.toLowerCase()) &&
            (await eagle.extraModule.ffmpeg.isInstalled()) &&
            !isRecursion &&
            config.readVideo === "read"
        ) {
            console.log("正在截取视频: " + imageItemsBatchs[i].filePath);
            const inputFile = imageItemsBatchs[i].filePath;
            const ffmpegPaths = await eagle.extraModule.ffmpeg.getPaths();
            const ffmpegBinaryPath = ffmpegPaths.ffmpeg;
            const ffprobeBinaryPath = ffmpegPaths.ffprobe;
            ffmpeg.setFfmpegPath(ffmpegBinaryPath);
            ffmpeg.setFfprobePath(ffprobeBinaryPath);
            const duration = await getVideoDuration(inputFile);
            const percentages = [1, 20, 40, 60, 80, 99];
            const timestamps = percentages.map((percent) => {
                return (duration * percent) / 100;
            });
            for (let i in timestamps) {
                const savedFiles = await takeScreenshots(inputFile, __dirname, [
                    i,
                ]);
                const tag1 = await getTag(
                    savedFiles,
                    session,
                    tagSet,
                    config,
                    [imageItemsBatchs[i]],
                    modelInfo,
                    true
                );
                tag1[0].forEach((element) => {
                    if (!tag.includes(element)) {
                        tag.push(element);
                    }
                });
            }
            fs.unlinkSync(path.join(__dirname, "video.png"));
        }
        ans.push(tag);
    }
    for (let i = 0; i < occupy.length; i++) {
        if (occupy[i] === 0) {
            ans.splice(i, 0, []);
        }
    }
    return ans;
}

module.exports = async function aiTagger(images, imageItems, setTag, config) {
    // 计算时间
    console.time("aiTagger");
    let modelInfo;
    if (config.modelPath.toLowerCase().includes("wd")) {
        modelInfo = new wdModel(config.modelPath);
    } else if (config.modelPath.toLowerCase().includes("cl")) {
        modelInfo = new clModel(config.modelPath);
    } else if (config.modelPath.toLowerCase().includes("ca")) {
        modelInfo = new caModel(config.modelPath);
    }
    const session = await createSession(modelInfo.modelPath);
    const tagSet = await modelInfo.checkTagsFileExists();
    let step = config.steps;
    let imageBatchs = [];
    let imageItemsBatchs = [];
    let countStep = 0;
    for (let i = 0; i < images.length; i++) {
        console.log(`正在处理第${i + 1}/${images.length}张图片: ${images[i]}`);
        if (config.overwrite !== "nocover" || imageItems[i].tags.length === 0) {
            imageBatchs.push(images[i]);
            imageItemsBatchs.push(imageItems[i]);
            countStep++;
        } else {
            window.completeItem.value++;
            continue;
        }
        if (step == countStep) {
            const tag = await getTag(
                imageBatchs,
                session,
                tagSet,
                config,
                imageItemsBatchs,
                modelInfo
            );
            await setTag(imageItemsBatchs, tag);
            countStep = 0;
            imageBatchs = [];
            imageItemsBatchs = [];
        }
    }
    if (countStep > 0) {
        const tag = await getTag(
            imageBatchs,
            session,
            tagSet,
            config,
            imageItemsBatchs,
            modelInfo
        );
        await setTag(imageItemsBatchs, tag);
        countStep = 0;
        imageBatchs = [];
        imageItemsBatchs = [];
    }
    console.timeEnd("aiTagger");
    return true;
};
