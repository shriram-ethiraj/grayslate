import { createHash } from "node:crypto";
import { copyFile, readFile, readdir, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

function option(name, fallback) {
    const index = process.argv.indexOf(`--${name}`);
    return index === -1 ? fallback : process.argv[index + 1];
}

const version = option("version");
const directory = resolve(option("directory"));
const repository = option("repository", process.env.GITHUB_REPOSITORY);
const publishedAt = option("published-at", new Date().toISOString());

if (!version || !repository) {
    throw new Error("--version and --repository (or GITHUB_REPOSITORY) are required.");
}

const assets = {
    "macos-universal": `Grayslate-${version}-macos-universal.app.tar.gz`,
    "linux-x86_64": `Grayslate-${version}-linux-x86_64.AppImage`,
    "windows-x86_64": `Grayslate-${version}-windows-x86_64-setup.exe`,
    "windows-aarch64": `Grayslate-${version}-windows-aarch64-setup.exe`,
};

const stableAliases = new Map([
    [`Grayslate-${version}-macos-universal.dmg`, "grayslate-macos-universal.dmg"],
    [
        `Grayslate-${version}-windows-x86_64-setup.exe`,
        "grayslate-windows-x86_64-setup.exe",
    ],
    [
        `Grayslate-${version}-windows-aarch64-setup.exe`,
        "grayslate-windows-aarch64-setup.exe",
    ],
    [
        `Grayslate-${version}-linux-x86_64.AppImage`,
        "grayslate-linux-x86_64.AppImage",
    ],
    [`Grayslate-${version}-linux-x86_64.deb`, "grayslate-linux-x86_64.deb"],
    [`Grayslate-${version}-linux-x86_64.rpm`, "grayslate-linux-x86_64.rpm"],
]);

for (const [source, alias] of stableAliases) {
    await copyFile(join(directory, source), join(directory, alias));
}

const platforms = {};
for (const [platform, filename] of Object.entries(assets)) {
    const signature = (await readFile(join(directory, `${filename}.sig`), "utf8")).trim();
    if (!signature) {
        throw new Error(`Signature is empty for ${filename}.`);
    }
    platforms[platform] = {
        signature,
        url: `https://github.com/${repository}/releases/download/v${version}/${filename}`,
    };
}

const latest = {
    version,
    notes: `See the Grayslate ${version} release notes on GitHub.`,
    pub_date: publishedAt,
    platforms,
};
await writeFile(join(directory, "latest.json"), `${JSON.stringify(latest, null, 2)}\n`);

const filenames = (await readdir(directory))
    .filter((filename) => filename !== "SHA256SUMS")
    .sort((left, right) => left.localeCompare(right));
const checksumLines = [];
for (const filename of filenames) {
    const data = await readFile(join(directory, filename));
    const digest = createHash("sha256").update(data).digest("hex");
    checksumLines.push(`${digest}  ${filename}`);
}
await writeFile(join(directory, "SHA256SUMS"), `${checksumLines.join("\n")}\n`);

console.log(`Assembled latest.json and SHA256SUMS for ${filenames.length} release assets.`);
