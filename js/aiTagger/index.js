const ort = require("onnxruntime-node");
const csv = require("csv-parser");
const fs = require("fs");
const sharp = require("sharp");
const path = require("path");

async function getCsvData(filePath) {
	const result = [];
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
}
async function createSession(modelPath) {
	return ort.InferenceSession.create(modelPath, {
		executionProviders: [{ name: "dml" }, { name: "cpu" }, { name: "cuda", deviceId: 0 }],
		graphOptimizationLevel: "all",
		executionMode: "parallel",
	});
}

const preprocessImage = async (imagePath) => {
	let image = sharp(imagePath);
	try {
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
	} catch (error) {
		console.log("无法读取图像", imagePath);
		return [];
	}
};
async function runInference(imagePath) {
	const results = [];
	for (let i = 0; i < imagePath.length; i++) {
		const floatData = await preprocessImage(imagePath[i]);
		if (floatData.length === 0) {
			results.push([]);
			continue;
		}
		const tensor = new ort.Tensor("float32", floatData, [1, sizeSetting, sizeSetting, 3]);
		results.push(tensor);
	}
	return results;
}
async function getTag(imagePath) {
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
	const batchTensor = new ort.Tensor("float32", concatenatedData, [imagePath.length, sizeSetting, sizeSetting, 3]);
	const feeds = {};
	if (session.inputNames[0]) {
		feeds[session.inputNames[0]] = batchTensor;
	}
	const result = await session.run(feeds);
	if (result === undefined) return [];
	console.log(result);
	const cpuData = result.output?.data || result.output?.cpuData || result.predictions_sigmoid?.cpuData;
	const perBatchSize = cpuData.length / imagePath.length;
	let ans = [];
	for (let i = 0; i < imagePath.length; i++) {
		let tag = [];
		for (let j = 0; j < perBatchSize; j++) {
			if (cpuData[i * perBatchSize + j] > threshold) {
				tag.push(tagSet[j].name);
			}
		}
		ans.push(tag);
	}
	return ans;
}

// 读取json文件
const sizeSetting = 448;
let session, tagSet, threshold;

module.exports = async function aiTagger(images, imageItems, setTag, config, overwrite) {
	// 计算时间
	session = await createSession(path.join(__dirname, "..", "..", "models", config.modelPath, "model.onnx"));
	tagSet = await getCsvData(path.join(__dirname, "..", "..", "models", config.modelPath, "selected_tags.csv"));
	threshold = config.threshold;
	console.time("计算时间");
	for (let i = 0; i < images.length; i++) {
		console.log(`正在处理第${i + 1}/${images.length}张图片: ${images[i]}`);
		console.log(imageItems[i].tags);
		if (overwrite || imageItems[i].tags.length === 0) {
			const tag = await getTag([images[i]]);
			await setTag(imageItems[i], tag);
		} else {
			window.completeItem.value++;
		}
	}
	console.timeEnd("计算时间");
	return true;
};
