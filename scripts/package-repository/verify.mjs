import { spawn } from "node:child_process";
import { access, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { parseArguments, sha256, walkFiles } from "./lib.mjs";

const args = parseArguments(process.argv.slice(2));
const root = resolve(args.get("directory") ?? "package-repository");
const keyring = join(root, "keys", "grayslate-archive-keyring.gpg");
const required = [
    "apt/dists/stable/InRelease",
    "apt/dists/stable/Release",
    "apt/dists/stable/Release.gpg",
    "apt/dists/stable/main/binary-amd64/Packages",
    "apt/dists/stable/main/dep11/Components-amd64.yml.gz",
    "rpm/stable/x86_64/repodata/repomd.xml",
    "rpm/stable/x86_64/repodata/repomd.xml.asc",
    "config/grayslate.sources",
    "config/grayslate.repo",
    "install.sh",
];
for (const path of required) await access(join(root, path));

await command("gpgv", ["--keyring", keyring, join(root, "apt/dists/stable/InRelease")]);
await command("gpgv", ["--keyring", keyring, join(root, "rpm/stable/x86_64/repodata/repomd.xml.asc"), join(root, "rpm/stable/x86_64/repodata/repomd.xml")]);

const packageIndex = await readFile(join(root, "apt/dists/stable/main/binary-amd64/Packages"), "utf8");
if (!/^Package: grayslate$/mu.test(packageIndex)) throw new Error("APT Packages does not contain grayslate");
const files = await walkFiles(root);
if (!files.some((path) => path.endsWith(".deb")) || !files.some((path) => path.endsWith(".rpm"))) {
    throw new Error("Repository is missing DEB or RPM packages");
}
await command("sh", ["-n", join(root, "install.sh")]);
const installer = await readFile(join(root, "install.sh"), "utf8");
if (installer.includes("@GRAYSLATE_")) throw new Error("Installer contains an unresolved checksum placeholder");
for (const expected of [
    "$GRAYSLATE_PACKAGES_URL/config/grayslate.sources",
    "$GRAYSLATE_PACKAGES_URL/config/grayslate.repo",
    "repo_gpgcheck=1",
]) {
    if (!installer.includes(expected)) throw new Error(`Installer is missing required policy: ${expected}`);
}
for (const path of [
    "keys/grayslate-archive-keyring.gpg",
    "keys/grayslate-archive-key.asc",
    "config/grayslate.sources",
    "config/grayslate.repo",
]) {
    const hash = await sha256(join(root, path));
    if (!installer.includes(hash)) throw new Error(`Installer does not pin the published checksum for ${path}`);
}

function command(executable, arguments_) {
    return new Promise((resolveCommand, reject) => {
        const child = spawn(executable, arguments_, { stdio: "inherit" });
        child.on("error", reject);
        child.on("close", (code) => code === 0 ? resolveCommand() : reject(new Error(`${executable} exited with ${code}`)));
    });
}
