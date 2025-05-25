const fs = require("fs").promises;
const path = require("path");
module.exports = async function readModel() {
	try {
		const dirents = await fs.readdir(__dirname + "/../../models", { withFileTypes: true });
		return dirents.filter((dirent) => dirent.isDirectory()).map((dirent) => dirent.name);
	} catch (err) {
		console.error("读取目录出错:", err);
		return [];
	}
};
