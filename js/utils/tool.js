const fs = require("fs-extra");

let is_processing = false;
let count_taggger = 0;
let error_image = [];

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
    console.log(tagListBatch);
    for (let i = 0; i < imageItems.length; i++) {
        window.completeItem.value++;
        count_taggger++;
        if (tagListBatch[i].length === 0) {
            error_image.push(imageItems[i].name);
            continue;
        }
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
                if (tagger[tag] !== undefined)
                    return `${tagger[tag]}${window.formData.value.splitter}${tag}`;
            return tag;
        });
        const tagListFiltered = tagList.filter(
            (tag) => tag != "" && tag != null && tag != undefined
        );
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
    count_taggger = 0;
    window.items.forEach((item) => {
        imagePath.push(item.thumbnailPath);
    });
    try {
        const aiTagger = require(__dirname + "/js/utils/ai-tagger.js");
        await notification(`正在开始打标，有${imagePath.length}个文件待处理`);
        await aiTagger(imagePath, window.items, setTag, config);
    } catch (e) {
        alert("有部分文件处理失败，请关闭窗口或者调低批次大小后重试");
        console.error(e);
    }
    if (error_image.length > 0) {
        sendNotification.dialog.error({
            title: "有部分文件处理失败",
            content: () => {
                return h("div", [
                    h("div", "失败文件："),
                    error_image.map((fileName) =>
                        h("div", { style: "color: red" }, fileName)
                    ),
                ]);
            },
        });
    } else {
        await notification(`所有文件已打标完成,共${count_taggger}个文件`);
    }
    is_processing = false;
}

 async function initialize(isAll) {
    // 初始化
    let items = await eagle.item.getSelected();
    if (items.length === 0 || isAll) {
        items = await eagle.item.getAll();
    }
    error_image = [];
    window.items = items;
    window.allItem.value = items.length;
    window.completeItem.value = 0;
}

 async function checkConfigPath() {
    // 检查config.private.json是否存在
    if (fs.existsSync(__dirname + "/config.private.json")) {
        return __dirname + "/config.private.json";
    }
    return __dirname + "/config.json";
}

 async function getConfig() {
    // 获取配置
    return JSON.parse(fs.readFileSync(await checkConfigPath(), "utf-8"));
}

 async function setConfig(config) {
    // 设置配置
    fs.writeFileSync(await checkConfigPath(), JSON.stringify(config, null, 2));
}

 async function autotaggerFun(config) {
    if (config.autotagger === true) {
        let tmpData = JSON.parse(JSON.stringify(config));
        config.overwrite = "nocover";
        window.formData.value = config;
        await window.handleSubmit(true);
        config = tmpData;
        window.formData.value = tmpData;
    }
}
