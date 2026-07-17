import { readFile } from "node:fs/promises";

const root = new URL("../../", import.meta.url);
const tag = process.argv[2] ?? process.env.GITHUB_REF_NAME;

if (!tag || !/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag)) {
    throw new Error(`Expected a release tag such as v1.2.3; received ${tag ?? "nothing"}.`);
}

const version = tag.slice(1);
const packageJson = JSON.parse(await readFile(new URL("package.json", root), "utf8"));
const tauriConfig = JSON.parse(
    await readFile(new URL("src-tauri/tauri.conf.json", root), "utf8"),
);
const cargoToml = await readFile(new URL("src-tauri/Cargo.toml", root), "utf8");
const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const appstream = await readFile(
    new URL("packaging/linux/app.grayslate.Grayslate.metainfo.xml", root),
    "utf8",
);
const appstreamVersion = appstream.match(/<release version="([^"]+)"/u)?.[1];

const versions = new Map([
    ["package.json", packageJson.version],
    ["src-tauri/tauri.conf.json", tauriConfig.version],
    ["src-tauri/Cargo.toml", cargoVersion],
    ["Linux AppStream metadata", appstreamVersion],
]);

for (const [file, configuredVersion] of versions) {
    if (configuredVersion !== version) {
        throw new Error(`${file} has version ${configuredVersion}; expected ${version} from ${tag}.`);
    }
}

if (tauriConfig.identifier !== "app.grayslate.Grayslate") {
    throw new Error("The release identifier must be app.grayslate.Grayslate.");
}

if (tauriConfig.mainBinaryName !== "Grayslate") {
    throw new Error(
        "mainBinaryName must be Grayslate for deterministic application executable naming.",
    );
}

const binarySections = cargoToml
    .split(/^\[\[bin\]\]\s*$/m)
    .slice(1)
    .map((section) => section.split(/^\[/m, 1)[0]);
const namingAuditTarget = binarySections.find((section) =>
    /^name\s*=\s*"name_file"\s*$/m.test(section),
);
if (
    !namingAuditTarget ||
    !/^required-features\s*=\s*\[\s*"naming-audit-cli"\s*\]\s*$/m.test(
        namingAuditTarget,
    )
) {
    throw new Error(
        "The name_file audit binary must require naming-audit-cli so Tauri excludes it from release bundles.",
    );
}

const updater = tauriConfig.plugins?.updater;
const publicKey = updater?.pubkey;
if (
    typeof publicKey !== "string" ||
    publicKey.includes("REPLACE_WITH") ||
    publicKey.length < 40
) {
    throw new Error(
        "Commit the real Tauri updater public key to src-tauri/tauri.conf.json before releasing.",
    );
}

const expectedEndpoint =
    "https://github.com/shriram-ethiraj/grayslate/releases/latest/download/latest.json";
if (!Array.isArray(updater.endpoints) || !updater.endpoints.includes(expectedEndpoint)) {
    throw new Error(`The updater endpoint must include ${expectedEndpoint}.`);
}

if (tauriConfig.bundle?.createUpdaterArtifacts !== true) {
    throw new Error("bundle.createUpdaterArtifacts must be true for direct-download releases.");
}

console.log(`Release inputs are consistent for Grayslate ${version}.`);
