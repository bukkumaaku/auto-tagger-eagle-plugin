const axios = require('axios');
const { on } = require('events');
const fs = require('fs-extra');
const path = require('path');

const BASE_URL = 'https://hf-mirror.com/SmilingWolf/';
const MODEL_DIR = __dirname + "/../../models";
const DOWNLOAD_LIST = {
    "wd":{
        model:"model.onnx",
        tags:"selected_tags.csv"
    },
    "cl":{
        model:"model.onnx",
        tags:"mapping_tags.csv"
    }
};

async function downloadFile(modelName, type, onProgress) {
    await fs.ensureDir(path.join(MODEL_DIR, modelName));
    const modelFileName = DOWNLOAD_LIST[type].model;
    const tagsFileName = DOWNLOAD_LIST[type].tags;
    const modelUrl = `${BASE_URL}${modelName}/resolve/main/${modelFileName}`;
    const tagsUrl = `${BASE_URL}${modelName}/resolve/main/${tagsFileName}`;
    if(await fs.pathExists(path.join(MODEL_DIR, modelName, modelFileName)) && await fs.pathExists(path.join(MODEL_DIR, modelName, tagsFileName))) {
        onProgress(100,"0 MB/s","tags");
        onProgress(100,"0 MB/s","model");
        return;
    }
    else{
        await fs.remove(path.join(MODEL_DIR, modelName));
        await fs.ensureDir(path.join(MODEL_DIR, modelName));
    }
    
    await downloadProcess(tagsUrl,modelName,tagsFileName,onProgress);
    await downloadProcess(modelUrl,modelName,modelFileName,onProgress);
}

async function downloadProcess(url,modelName,fileName,onProgress){
    const { data, headers} = await axios({tagsUrl, method: 'GET', responseType: 'stream' });
    const totalLength = headers['content-length'];
    let downloadedLength = 0;
    const startTime = Date.now();
    return new Promise((resolve, reject) => {
        const writer = fs.createWriteStream(path.join(MODEL_DIR, modelName, fileName));
        data.on('data', (chunk) => {
            downloadedLength += chunk.length;
            const elapsedTime = (Date.now() - startTime) / 1000;
            const speed = (downloadedLength / 1024/ 1024 / elapsedTime).toFixed(2);
            const progress = ((downloadedLength / totalLength) * 100).toFixed(2);
            onProgress(progress,speed + " MB/s",fileName.endsWith(".onnx")?"model":"tags");
        });
        writer.on('finish', resolve);
        writer.on('error', reject);
        data.pipe(writer);
    });
}