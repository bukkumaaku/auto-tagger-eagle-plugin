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
