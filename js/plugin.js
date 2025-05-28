const fs = require("fs");

let is_processing = false;

async function wait(ms) {
	return new Promise((resolve) => {
		setTimeout(resolve, ms);
	});
}

async function setTag(imageItems, tagListBatch) {
	for (let i = 0; i < imageItems.length; i++) {
		window.completeItem.value++;
		const originalTags = imageItems[i].tags;
		const tagList = tagListBatch[i].map((tag) => {
			if (window.formData.value.language === "zh") if (tagger[tag] !== undefined) return tagger[tag];
			if (window.formData.value.language === "mix")
				if (tagger[tag] !== undefined) return `${tagger[tag]}${window.formData.value.splitter}${tag}`;
			return tag;
		});
		const tagListFiltered = tagList.filter((tag) => !window.formData.value.filterTags.includes(tag));
		if (window.formData.value.overwrite === "merge") {
			originalTags.forEach((tag) => {
				if (!tagListFiltered.includes(tag)) tagListFiltered.push(tag);
			});
		}
		imageItems[i].tags = tagListFiltered;
		await imageItems[i].save();
	}
}

async function startGetTag(config) {
	//let items = await eagle.item.getSelected();
	const imagePath = [];
	window.items.forEach((item) => {
		imagePath.push(item.thumbnailPath);
	});
	try {
		const aiTagger = require(__dirname + "/js/utils/ai-tagger.js");
		await aiTagger(imagePath, window.items, setTag, config, window.formData.value.overwrite);
	} catch (e) {
		alert("有部分文件处理失败，请关闭窗口后重试");
	}
	await wait(500);

	sendNotification.notification.success({ title: "已全部打标结束", duration: 3000 });
	await eagle.notification.show({
		title: "成功",
		body: "所有文件已打标完成",
		mute: false,
		duration: 3000,
	});
	is_processing = false;
}

async function initialize() {
	let items = await eagle.item.getSelected();
	if (items.length === 0) {
		items = await eagle.item.getAll();
	}
	window.items = items;
	window.allItem.value = items.length;
	window.completeItem.value = 0;
}

async function getConfig() {
	return JSON.parse(fs.readFileSync(__dirname + "/config.json", "utf-8"));
}

async function setConfig(config) {
	fs.writeFileSync(__dirname + "/config.json", JSON.stringify(config, null, 2));
}
eagle.onPluginCreate(() => {
	console.log("eagle.onPluginCreate");
});

eagle.onPluginRun(async () => {
	console.log("eagle.onPluginRun");
	const modelsList = await require(__dirname + "/js/utils/models.js")();
	window.options.value = [];
	modelsList.forEach((model) => {
		window.options.value.push({ label: model, value: model });
	});
	await initialize();
});

eagle.onPluginShow(async () => {
	console.log("eagle.onPluginShow");
});

eagle.onPluginHide(() => {
	console.log("eagle.onPluginHide");
});

eagle.onPluginBeforeExit(() => {
	console.log("eagle.onPluginBeforeExit");
});
