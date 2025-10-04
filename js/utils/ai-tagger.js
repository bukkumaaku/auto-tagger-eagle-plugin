const ort = require("onnxruntime-node");
const csv = require("csv-parser");
const fs = require("fs");
const sharp = require("sharp");
const path = require("path");
const ffmpeg = require("fluent-ffmpeg");
const video_ext = ["mp4", "avi", "mov", "mkv", "flv", "wmv"];

let wd_or_cl = "wd";

async function getCsvData(filePath) {
	const result = [];
	if (fs.existsSync(path.join(filePath, "selected_tags.csv"), fs.constants.R_OK)) {
		wd_or_cl = "wd";
		filePath = path.join(filePath, "selected_tags.csv");
		const csvData = await new Promise((resolve, reject) => {
			fs.createReadStream(filePath)
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
	} else if (fs.existsSync(path.join(filePath, "tag_mapping.json"), fs.constants.R_OK)) {
		wd_or_cl = "cl";
		filePath = path.join(filePath, "tag_mapping.json");

		const config = require(filePath);
		const len = Object.keys(config).length;
		for (let i = 0; i < len; i++) {
			result.push({ name: config[i].tag });
		}
	}
	return result;
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

function postProcessLogits(logitsData) {
	const CLIP_MIN = -30.0;
	const CLIP_MAX = 30.0;
	const probabilities = logitsData.map((logit) => {
		let value = logit;
		if (isNaN(value)) {
			value = 0.0;
		} else if (value === Infinity) {
			value = 1.0;
		} else if (value === -Infinity) {
			value = 0.0;
		}
		const clippedValue = Math.max(CLIP_MIN, Math.min(CLIP_MAX, value));
		return 1 / (1 + Math.exp(-clippedValue));
	});
	return Float32Array.from(probabilities);
}

async function preprocessImage(imagePath) {
	try {
		let image = sharp(imagePath);
		if (wd_or_cl === "cl") {
			image = image.flatten({ background: "#000000" });
			image = image.resize({
				width: sizeSetting,
				height: sizeSetting,
				fit: "contain",
				background: { r: 0, g: 0, b: 0 },
			});
			const rawPixelData = await image.raw().toBuffer();
			const channels = 3;
			const inputData = new Float32Array(sizeSetting * sizeSetting * channels);

			const mean = 0.5;
			const std = 0.5;
			for (let y = 0; y < sizeSetting; y++) {
				for (let x = 0; x < sizeSetting; x++) {
					const i = (y * sizeSetting + x) * channels;
					const r = rawPixelData[i];
					const g = rawPixelData[i + 1];
					const b = rawPixelData[i + 2];
					const normalized_b = (b / 255.0 - mean) / std;
					const normalized_g = (g / 255.0 - mean) / std;
					const normalized_r = (r / 255.0 - mean) / std;
					inputData[y * sizeSetting + x] = normalized_b;
					inputData[sizeSetting * sizeSetting + y * sizeSetting + x] = normalized_g;
					inputData[2 * sizeSetting * sizeSetting + y * sizeSetting + x] = normalized_r;
				}
			}

			return inputData;
		} else {
			image = image.flatten();
			image = image.resize({
				width: sizeSetting,
				height: sizeSetting,
				fit: "contain",
				background: { r: 0, g: 0, b: 0 },
			});
			const channels = 3;
			const imageData = await image.raw().toBuffer();
			const reshapedData = new Uint8Array(sizeSetting * sizeSetting * channels);
			for (let i = 0; i < imageData.length; i += 3) {
				for (let channel = 0; channel < 3; channel++) {
					reshapedData[i + 2 - channel] = imageData[i + channel] || 0;
				}
			}
			const inputData = Float32Array.from(reshapedData);
			return inputData;
		}
	} catch (error) {
		console.error("无法处理图像:", imagePath, error);
		return [];
	}
}
async function runInference(imagePath) {
	const results = [];
	for (let i = 0; i < imagePath.length; i++) {
		const floatData = await preprocessImage(imagePath[i]);
		if (floatData.length === 0) {
			results.push([]);
			continue;
		}
		let arr = [1, sizeSetting, sizeSetting, 3];
		if (wd_or_cl === "cl") {
			arr = [1, 3, sizeSetting, sizeSetting];
		}
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
				filenames.forEach((name) => outputFilenames.push(path.join(outputFolder, name)));
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

async function getTag(imagePath, session, tagSet, config, imageItemsBatchs, isRecursion = false) {
	const results = await runInference(imagePath);
	let resultsLength = results.length;
	if (results[0]?.data === undefined) return [];
	for (let i = 0; i < resultsLength; i++) {
		if (results[i] === undefined) {
			imagePath.splice(i, 1);
			results.splice(i, 1);
			i--;
			resultsLength--;
			continue;
		}
	}
	if (imagePath.length === 0) return [];
	const concatenatedData = new Float32Array(imagePath.length * 3 * sizeSetting * sizeSetting);
	let offset = 0;
	for (const tensor of results) {
		concatenatedData.set(tensor.data, offset);
		offset += tensor.data.length;
	}
	// 运行模型
	let arr = [imagePath.length, sizeSetting, sizeSetting, 3];
	if (wd_or_cl === "cl") {
		arr = [imagePath.length, 3, sizeSetting, sizeSetting];
	}
	const batchTensor = new ort.Tensor("float32", concatenatedData, arr);
	const feeds = {};
	if (session.inputNames[0]) {
		feeds[session.inputNames[0]] = batchTensor;
	}
	const result = await session.run(feeds);
	if (result === undefined) return [];
	let cpuData =
		result.output?.data || result.output?.cpuData || result.predictions_sigmoid?.cpuData || result.output0.cpuData;
	if (wd_or_cl === "cl") {
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
	}
	const perBatchSize = cpuData.length / imagePath.length;
	let ans = [];
	for (let i = 0; i < imagePath.length; i++) {
		let tag = [];
		for (let j = 0; j < perBatchSize; j++) {
			if (cpuData[i * perBatchSize + j] > config.threshold) {
				tag.push(tagSet[j].name);
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
				const savedFiles = await takeScreenshots(inputFile, __dirname, [i]);
				const tag1 = await getTag(savedFiles, session, tagSet, config, [imageItemsBatchs[i]], true);
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
	return ans;
}

// 读取json文件
const sizeSetting = 448;

module.exports = async function aiTagger(images, imageItems, setTag, config) {
	// 计算时间
	console.time("aiTagger");
	const session = await createSession(path.join(__dirname, "..", "..", "models", config.modelPath, "model.onnx"));
	const tagSet = await getCsvData(path.join(__dirname, "..", "..", "models", config.modelPath));
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
			const tag = await getTag(imageBatchs, session, tagSet, config, imageItemsBatchs);
			await setTag(imageItemsBatchs, tag);
			countStep = 0;
			imageBatchs = [];
			imageItemsBatchs = [];
		}
	}
	if (countStep > 0) {
		const tag = await getTag(imageBatchs, session, tagSet, config, imageItemsBatchs);
		await setTag(imageItemsBatchs, tag);
		countStep = 0;
		imageBatchs = [];
		imageItemsBatchs = [];
	}
	console.timeEnd("aiTagger");
	return true;
};
