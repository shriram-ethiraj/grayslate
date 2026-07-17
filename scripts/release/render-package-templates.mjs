import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

function option(name) {
    const index = process.argv.indexOf(`--${name}`);
    if (index === -1 || !process.argv[index + 1]) {
        throw new Error(`Missing --${name}.`);
    }
    return process.argv[index + 1];
}

async function sha256(path) {
    return createHash("sha256").update(await readFile(path)).digest("hex");
}

const version = option("version");
const assets = resolve(option("assets"));
const output = resolve(option("output"));
const root = new URL("../../", import.meta.url);

const sourceSha = await sha256(join(assets, `Grayslate-${version}-source.tar.gz`));
const dmgSha = await sha256(
    join(assets, `Grayslate-${version}-macos-universal-homebrew.dmg`),
);
const replacements = new Map([
    ["@VERSION@", version],
    ["@SOURCE_SHA256@", sourceSha],
    ["@DMG_SHA256@", dmgSha],
]);

const templates = [
    ["packaging/flatpak/app.grayslate.Grayslate.yml.in", "app.grayslate.Grayslate.yml"],
    ["packaging/aur/PKGBUILD.in", "PKGBUILD"],
    ["packaging/homebrew/Casks/grayslate.rb.in", "grayslate.rb"],
];

await mkdir(output, { recursive: true });
for (const [source, destination] of templates) {
    let content = await readFile(new URL(source, root), "utf8");
    for (const [token, value] of replacements) {
        content = content.replaceAll(token, value);
    }
    if (/@[A-Z0-9_]+@/u.test(content)) {
        throw new Error(`Unresolved template token in ${source}.`);
    }
    await writeFile(join(output, destination), content);
    console.log(`Rendered ${destination}.`);
}
