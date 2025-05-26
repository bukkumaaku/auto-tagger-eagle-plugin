const fs = require("fs");

let is_processing = false;

async function wait(ms) {
	return new Promise((resolve) => {
		setTimeout(resolve, ms);
	});
}

async function setTag(imageItem, tagListBatch) {
	window.completeItem.value++;
	const tagList = tagListBatch[0].map((tag) => {
		if (window.formData.value.language === "zh") if (tagger[tag] !== undefined) return tagger[tag];
		if (window.formData.value.language === "mix")
			if (tagger[tag] !== undefined) return `${tagger[tag]}${window.formData.value.splitter}${tag}`;
		return tag;
	});
	const tagListFiltered = tagList.filter((tag) => !window.formData.value.filterTags.includes(tag));
	imageItem.tags = tagListFiltered;
	await imageItem.save();
}

async function startGetTag(config) {
	if (is_processing) {
		console.log("Please wait for the previous process to complete.");
		return;
	}
	is_processing = true;
	const items = await eagle.item.getSelected();
	const imagePath = [];
	items.forEach((item) => {
		imagePath.push(item.thumbnailPath);
	});
	const aiTagger = require(__dirname + "\\js\\utils\\ai-tagger.js");
	await aiTagger(imagePath, items, setTag, config, window.formData.value.overwrite);
	await wait(500);
	alert("已完成");
	is_processing = false;
}

async function getConfig() {
	return JSON.parse(fs.readFileSync(__dirname + "\\config.json", "utf-8"));
}

async function setConfig(config) {
	fs.writeFileSync(__dirname + "\\config.json", JSON.stringify(config, null, 2));
}
eagle.onPluginCreate(() => {
	console.log("eagle.onPluginCreate");
});

eagle.onPluginRun(async () => {
	is_processing = false;
	console.log("eagle.onPluginRun");
	const modelsList = await require(__dirname + "\\js\\utils\\models.js")();
	window.options.value = [];
	modelsList.forEach((model) => {
		window.options.value.push({ label: model, value: model });
	});
	const items = await eagle.item.getSelected();
	window.allItem.value = items.length;
	window.completeItem.value = 0;
});

eagle.onPluginShow(async () => {
	console.log("eagle.onPluginShow");
});

eagle.onPluginHide(() => {
	console.log("eagle.onPluginHide");
	is_processing = false;
});

eagle.onPluginBeforeExit(() => {
	console.log("eagle.onPluginBeforeExit");
});
