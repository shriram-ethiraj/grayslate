import { cp, mkdir, readdir, rm } from "node:fs/promises";
import { basename, join, resolve } from "node:path";

function option(name) {
    const index = process.argv.indexOf(`--${name}`);
    if (index === -1 || !process.argv[index + 1]) {
        throw new Error(`Missing --${name}.`);
    }
    return process.argv[index + 1];
}

async function walk(directory) {
    const entries = await readdir(directory, { withFileTypes: true });
    const files = [];
    for (const entry of entries) {
        const path = join(directory, entry.name);
        if (entry.isDirectory()) {
            files.push(...(await walk(path)));
        } else if (entry.isFile()) {
            files.push(path);
        }
    }
    return files;
}

const target = option("target");
const version = option("version");
const bundleRoot = resolve(option("bundle-root"));
const output = resolve(option("output"));

if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Invalid version: ${version}`);
}

const targetDefinitions = {
    "macos-universal": [
        [".app.tar.gz", `Grayslate-${version}-macos-universal.app.tar.gz`],
        [".app.tar.gz.sig", `Grayslate-${version}-macos-universal.app.tar.gz.sig`],
        [".dmg", `Grayslate-${version}-macos-universal.dmg`],
    ],
    "macos-universal-system": [
        [".dmg", `Grayslate-${version}-macos-universal-homebrew.dmg`],
    ],
    "linux-x86_64-direct": [
        [".AppImage", `Grayslate-${version}-linux-x86_64.AppImage`],
        [".AppImage.sig", `Grayslate-${version}-linux-x86_64.AppImage.sig`],
    ],
    "linux-x86_64-system": [
        [".deb", `Grayslate-${version}-linux-x86_64.deb`],
        [".rpm", `Grayslate-${version}-linux-x86_64.rpm`],
    ],
    "windows-x86_64": [
        [".exe", `Grayslate-${version}-windows-x86_64-setup.exe`],
        [".exe.sig", `Grayslate-${version}-windows-x86_64-setup.exe.sig`],
    ],
    "windows-aarch64": [
        [".exe", `Grayslate-${version}-windows-aarch64-setup.exe`],
        [".exe.sig", `Grayslate-${version}-windows-aarch64-setup.exe.sig`],
    ],
};

const definitions = targetDefinitions[target];
if (!definitions) {
    throw new Error(`Unsupported staging target: ${target}`);
}

const files = await walk(bundleRoot);
await rm(output, { recursive: true, force: true });
await mkdir(output, { recursive: true });

for (const [suffix, stagedName] of definitions) {
    const matches = files.filter((file) => {
        if (!file.endsWith(suffix)) return false;
        if (suffix === ".exe") return file.includes(`${join("bundle", "nsis")}`);
        return true;
    });
    if (matches.length !== 1) {
        throw new Error(
            `Expected exactly one ${suffix} for ${target}; found ${matches.length}: ${matches.map(basename).join(", ")}`,
        );
    }
    await cp(matches[0], join(output, stagedName));
    console.log(`${matches[0]} -> ${stagedName}`);
}
