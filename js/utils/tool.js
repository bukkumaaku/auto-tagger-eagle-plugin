const fs = require("fs");

let is_processing = false;

async function wait(ms) {
	return new Promise((resolve) => {
		setTimeout(resolve, ms);
	});
}

async function notification(message) {
	// 发送通知
	sendNotification.notification.info({ title: message, duration: 3000 });
	await eagle.notification.show({
		title: "通知",
		body: message,
		mute: false,
		duration: 3000,
	});
}

async function setTag(imageItems, tagListBatch) {
	// 打标签
	for (let i = 0; i < imageItems.length; i++) {
		window.completeItem.value++;
		const tagList = tagListBatch[i].map((tag) => {
			if (
				window.formData.value.filterTags.includes(tag) ||
				window.formData.value.filterTags.includes(tagger[tag])
			) {
				return "";
			}
			if (window.formData.value.language === "zh") {
				if (tagger[tag] !== undefined) return tagger[tag];
			} else if (window.formData.value.language === "mix")
				if (tagger[tag] !== undefined) return `${tagger[tag]}${window.formData.value.splitter}${tag}`;
			return tag;
		});
		const tagListFiltered = tagList.filter((tag) => tag != "" && tag != null && tag != undefined);
		if (window.formData.value.overwrite === "merge") {
			imageItems[i].tags.forEach((tag) => {
				if (!tagListFiltered.includes(tag)) tagListFiltered.push(tag);
			});
		}
		imageItems[i].tags = tagListFiltered;
		await imageItems[i].save();
	}
}

async function startGetTag(config) {
	// 开始打标签
	const imagePath = [];
	window.items.forEach((item) => {
		imagePath.push(item.thumbnailPath);
	});
	try {
		const aiTagger = require(__dirname + "/js/utils/ai-tagger.js");
		await notification("正在加载模型，请稍候...");
		await aiTagger(imagePath, window.items, setTag, config);
	} catch (e) {
		alert("有部分文件处理失败，请关闭窗口或者调低批次大小后重试");
		console.error(e);
	}
	await notification("所有文件已打标完成");
	is_processing = false;
}

async function initialize() {
	// 初始化
	let items = await eagle.item.getSelected();
	if (items.length === 0) {
		items = await eagle.item.getAll();
	}
	window.items = items;
	window.allItem.value = items.length;
	window.completeItem.value = 0;
}

async function getConfig() {
	// 获取配置
	return JSON.parse(fs.readFileSync(__dirname + "/config.json", "utf-8"));
}

async function setConfig(config) {
	// 设置配置
	fs.writeFileSync(__dirname + "/config.json", JSON.stringify(config, null, 2));
}
