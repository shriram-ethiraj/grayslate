import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const arguments_ = process.argv.slice(2);
const version = arguments_.shift();
let root = fileURLToPath(new URL("../../", import.meta.url));
let date = new Date().toISOString().slice(0, 10);

while (arguments_.length > 0) {
    const option = arguments_.shift();
    const value = arguments_.shift();
    if (option === "--root" && value) {
        root = resolve(value);
    } else if (option === "--date" && value && /^\d{4}-\d{2}-\d{2}$/u.test(value)) {
        date = value;
    } else {
        throw new Error(`Unknown or incomplete option: ${option ?? "nothing"}`);
    }
}

if (!version || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/u.test(version)) {
    throw new Error("Usage: pnpm release:version <semver> [--date YYYY-MM-DD]");
}

async function replaceExactlyOnce(relativePath, pattern, replacement) {
    const path = resolve(root, relativePath);
    const contents = await readFile(path, "utf8");
    const matches = [...contents.matchAll(new RegExp(pattern.source, pattern.flags.includes("g") ? pattern.flags : `${pattern.flags}g`))];
    if (matches.length !== 1) {
        throw new Error(`${relativePath}: expected exactly one version field, found ${matches.length}.`);
    }
    await writeFile(path, contents.replace(pattern, replacement));
}

await replaceExactlyOnce(
    "package.json",
    /^(  "version": ")[^"]+(",)$/mu,
    `$1${version}$2`,
);
await replaceExactlyOnce(
    "src-tauri/tauri.conf.json",
    /^(  "version": ")[^"]+(",)$/mu,
    `$1${version}$2`,
);
await replaceExactlyOnce(
    "src-tauri/Cargo.toml",
    /^(version = ")[^"]+("$)/mu,
    `$1${version}$2`,
);
await replaceExactlyOnce(
    "Cargo.lock",
    /(\[\[package\]\]\nname = "Grayslate"\nversion = ")[^"]+("\n)/u,
    `$1${version}$2`,
);

const appstreamPath = resolve(root, "packaging/linux/app.grayslate.Grayslate.metainfo.xml");
const appstream = await readFile(appstreamPath, "utf8");
const escapedVersion = version.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
const existingRelease = new RegExp(
    `<release version="${escapedVersion}" date="[^"]+" \\/>`,
    "u",
);
const release = `<release version="${version}" date="${date}" />`;
let updatedAppstream;
if (existingRelease.test(appstream)) {
    updatedAppstream = appstream.replace(existingRelease, release);
} else if ((appstream.match(/<releases>/gu) ?? []).length === 1) {
    updatedAppstream = appstream.replace(/<releases>\n/u, `<releases>\n    ${release}\n`);
} else {
    throw new Error("Linux AppStream metadata must contain exactly one <releases> section.");
}
await writeFile(appstreamPath, updatedAppstream);

console.log(`Updated Grayslate release metadata to ${version}.`);
