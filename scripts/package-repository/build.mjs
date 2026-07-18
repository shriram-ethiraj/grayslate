import { spawn } from "node:child_process";
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { gzipSync } from "node:zlib";
import {
    missingRequiredPackageMetadata,
    newestStablePackage,
    parseArguments,
    sha256,
    walkFiles,
} from "./lib.mjs";

const args = parseArguments(process.argv.slice(2));
const packagesDirectory = resolve(required("packages"));
const output = resolve(required("output"));
const publicKeys = resolve(args.get("public-keys") ?? "packaging/repository/keys");
const fingerprint = process.env.LINUX_REPOSITORY_GPG_FINGERPRINT?.replaceAll(" ", "");

if (!fingerprint || !/^[0-9A-Fa-f]{40}$/u.test(fingerprint)) {
    throw new Error("LINUX_REPOSITORY_GPG_FINGERPRINT must be a 40-character fingerprint");
}
if (!process.env.GNUPGHOME) {
    throw new Error("GNUPGHOME must point to the isolated repository signing keyring");
}

await rm(output, { recursive: true, force: true });
await mkdir(output, { recursive: true });

const allPackages = await walkFiles(packagesDirectory);
const debs = allPackages.filter((path) => path.endsWith(".deb"));
const rpms = allPackages.filter((path) => path.endsWith(".rpm"));
if (debs.length === 0 || rpms.length === 0) {
    throw new Error("The package input must contain at least one DEB and one RPM");
}

const aptRoot = join(output, "apt");
const rpmRoot = join(output, "rpm", "stable", "x86_64");
const aptPool = join(aptRoot, "pool", "main", "g", "grayslate");
await mkdir(aptPool, { recursive: true });
await mkdir(rpmRoot, { recursive: true });

const debPackages = [];
for (const source of debs) {
    const packageName = (await command("dpkg-deb", ["-f", source, "Package"])).trim();
    const architecture = (await command("dpkg-deb", ["-f", source, "Architecture"])).trim();
    const version = (await command("dpkg-deb", ["-f", source, "Version"])).trim();
    if (packageName !== "grayslate" || architecture !== "amd64") {
        throw new Error(`${basename(source)} is not the expected grayslate amd64 package`);
    }
    debPackages.push({
        source,
        filename: basename(source),
        version,
        contents: await command("dpkg-deb", ["--contents", source]),
    });
}

const rpmPackages = [];
for (const source of rpms) {
    const identity = (await command("rpm", ["-qp", "--queryformat", "%{NAME} %{VERSION} %{RELEASE} %{ARCH}", source])).trim();
    const [name, version, release, architecture] = identity.split(" ");
    if (name !== "grayslate" || architecture !== "x86_64") {
        throw new Error(`${basename(source)} is not the expected grayslate x86_64 package`);
    }
    rpmPackages.push({
        source,
        filename: basename(source),
        version,
        release,
        contents: await command("rpm", ["-qlp", source]),
    });
}

const newestDeb = newestStablePackage(debPackages);
const newestRpm = newestStablePackage(rpmPackages);
if (newestDeb.version !== newestRpm.version) {
    throw new Error(`Newest DEB (${newestDeb.version}) and RPM (${newestRpm.version}) versions do not match`);
}
requirePackageMetadata(newestDeb);
requirePackageMetadata(newestRpm);

for (const package_ of debPackages) {
    await cp(package_.source, join(aptPool, `grayslate_${package_.version}_amd64.deb`));
}

for (const package_ of rpmPackages) {
    const destination = join(rpmRoot, `grayslate-${package_.version}-${package_.release}.x86_64.rpm`);
    await cp(package_.source, destination);
    await signRpm(destination);
}

await buildAptRepository();
await buildRpmRepository();
await publishConfigurationAndKeys();

async function buildAptRepository() {
    const binary = join(aptRoot, "dists", "stable", "main", "binary-amd64");
    const dep11 = join(aptRoot, "dists", "stable", "main", "dep11");
    await mkdir(binary, { recursive: true });
    await mkdir(join(dep11, "icons", "64x64"), { recursive: true });
    await mkdir(join(dep11, "icons", "128x128"), { recursive: true });

    const packages = await command("apt-ftparchive", ["packages", "pool"], { cwd: aptRoot });
    await writeFile(join(binary, "Packages"), packages);
    await compressIndex(join(binary, "Packages"));

    const metainfo = await readFile("packaging/linux/app.grayslate.Grayslate.metainfo.xml", "utf8");
    const component = metainfo
        .replace(/^<\?xml[^>]*>\s*/u, "")
        .replace("</component>", "  <pkgname>grayslate</pkgname>\n  <icon type=\"cached\">app.grayslate.Grayslate.png</icon>\n</component>");
    const catalog = `<?xml version="1.0" encoding="UTF-8"?>\n<components version="1.0" origin="grayslate">\n${component}\n</components>\n`;
    const catalogXml = join(dep11, "Components-amd64.xml");
    const catalogYaml = join(dep11, "Components-amd64.yml");
    await writeFile(catalogXml, catalog);
    await command("appstreamcli", ["convert", "--format=yaml", catalogXml, catalogYaml]);
    await rm(catalogXml);
    await compressIndex(catalogYaml);
    await cp("src-tauri/icons/64x64.png", join(dep11, "icons", "64x64", "app.grayslate.Grayslate.png"));
    await cp("src-tauri/icons/128x128.png", join(dep11, "icons", "128x128", "app.grayslate.Grayslate.png"));
    await command("tar", ["--sort=name", "--mtime=@0", "--owner=0", "--group=0", "-czf", join(dep11, "icons-64x64.tar.gz"), "-C", join(dep11, "icons"), "64x64"]);
    await command("tar", ["--sort=name", "--mtime=@0", "--owner=0", "--group=0", "-czf", join(dep11, "icons-128x128.tar.gz"), "-C", join(dep11, "icons"), "128x128"]);

    for (const path of [join(binary, "Packages"), join(binary, "Packages.gz"), join(binary, "Packages.xz"), join(dep11, "Components-amd64.yml.gz"), join(dep11, "Components-amd64.yml.xz")]) {
        const hash = await sha256(path);
        const destination = join(dirname(path), "by-hash", "SHA256", hash);
        await mkdir(dirname(destination), { recursive: true });
        await cp(path, destination);
    }

    const releaseArguments = [
        "-o", "APT::FTPArchive::Release::Origin=Grayslate",
        "-o", "APT::FTPArchive::Release::Label=Grayslate",
        "-o", "APT::FTPArchive::Release::Suite=stable",
        "-o", "APT::FTPArchive::Release::Codename=stable",
        "-o", "APT::FTPArchive::Release::Architectures=amd64",
        "-o", "APT::FTPArchive::Release::Components=main",
        "-o", "APT::FTPArchive::Release::Description=Official Grayslate packages",
        "-o", "APT::FTPArchive::Release::Acquire-By-Hash=yes",
        "release", "dists/stable",
    ];
    const release = await command("apt-ftparchive", releaseArguments, { cwd: aptRoot });
    const releasePath = join(aptRoot, "dists", "stable", "Release");
    await writeFile(releasePath, release);
    await gpg(["--armor", "--detach-sign", "--output", `${releasePath}.gpg`, releasePath]);
    await gpg(["--armor", "--clearsign", "--output", join(dirname(releasePath), "InRelease"), releasePath]);
}

async function buildRpmRepository() {
    await command("createrepo_c", ["--checksum", "sha256", rpmRoot]);
    const metadataDirectory = join(rpmRoot, "repodata");
    const catalogPath = join(output, ".appstream.xml");
    const metainfo = await readFile("packaging/linux/app.grayslate.Grayslate.metainfo.xml", "utf8");
    const component = metainfo
        .replace(/^<\?xml[^>]*>\s*/u, "")
        .replace("</component>", "  <pkgname>grayslate</pkgname>\n  <icon type=\"remote\">https://packages.grayslate.app/media/app.grayslate.Grayslate.png</icon>\n</component>");
    await writeFile(catalogPath, `<?xml version="1.0" encoding="UTF-8"?>\n<components version="1.0" origin="grayslate">\n${component}\n</components>\n`);
    const compressedCatalog = `${catalogPath}.gz`;
    await writeFile(compressedCatalog, gzipSync(await readFile(catalogPath), { level: 9, mtime: 0 }));
    await command("modifyrepo_c", ["--mdtype", "appstream", compressedCatalog, metadataDirectory]);
    await rm(catalogPath);
    await rm(compressedCatalog);
    const repomd = join(metadataDirectory, "repomd.xml");
    await gpg(["--armor", "--detach-sign", "--output", `${repomd}.asc`, repomd]);
}

async function publishConfigurationAndKeys() {
    await mkdir(join(output, "config"), { recursive: true });
    await mkdir(join(output, "keys"), { recursive: true });
    await mkdir(join(output, "media"), { recursive: true });
    await cp("packaging/repository/config/grayslate.sources", join(output, "config", "grayslate.sources"));
    await cp("packaging/repository/config/grayslate.repo", join(output, "config", "grayslate.repo"));
    let installer = await readFile("packaging/repository/install.sh", "utf8");
    const installerHashes = new Map([
        ["@GRAYSLATE_APT_KEY_SHA256@", await sha256(join(publicKeys, "grayslate-archive-keyring.gpg"))],
        ["@GRAYSLATE_APT_CONFIG_SHA256@", await sha256("packaging/repository/config/grayslate.sources")],
        ["@GRAYSLATE_RPM_KEY_SHA256@", await sha256(join(publicKeys, "grayslate-archive-key.asc"))],
        ["@GRAYSLATE_RPM_CONFIG_SHA256@", await sha256("packaging/repository/config/grayslate.repo")],
    ]);
    for (const [placeholder, hash] of installerHashes) {
        if (installer.split(placeholder).length !== 2) {
            throw new Error(`Installer must contain exactly one ${placeholder} placeholder`);
        }
        installer = installer.replace(placeholder, hash);
    }
    await writeFile(join(output, "install.sh"), installer);
    for (const filename of ["grayslate-archive-key.asc", "grayslate-archive-keyring.gpg"]) {
        await cp(join(publicKeys, filename), join(output, "keys", filename));
    }
    await cp("src-tauri/icons/icon.png", join(output, "media", "app.grayslate.Grayslate.png"));
}

async function compressIndex(path) {
    const contents = await readFile(path);
    await writeFile(`${path}.gz`, gzipSync(contents, { level: 9, mtime: 0 }));
    await command("xz", ["--compress", "--keep", "--force", "--threads=0", path]);
}

async function signRpm(path) {
    const arguments_ = [
        "--define", `_gpg_name ${fingerprint}`,
        "--define", `_gpg_path ${process.env.GNUPGHOME}`,
    ];
    const passphraseFile = process.env.LINUX_REPOSITORY_GPG_PASSPHRASE_FILE;
    if (passphraseFile) {
        arguments_.push("--define", `_gpg_sign_cmd_extra_args --pinentry-mode loopback --passphrase-file ${passphraseFile}`);
    }
    arguments_.push("--addsign", path);
    await command("rpmsign", arguments_);
    await command("rpm", ["--checksig", path]);
}

async function gpg(arguments_) {
    const common = ["--batch", "--yes", "--local-user", fingerprint, "--digest-algo", "SHA256", "--pinentry-mode", "loopback"];
    const passphraseFile = process.env.LINUX_REPOSITORY_GPG_PASSPHRASE_FILE;
    if (passphraseFile) common.push("--passphrase-file", passphraseFile);
    await command("gpg", [...common, ...arguments_]);
}

function required(name) {
    const value = args.get(name);
    if (!value) throw new Error(`Missing --${name}`);
    return value;
}

function requirePackageMetadata(package_) {
    const missing = missingRequiredPackageMetadata(package_.contents);
    if (missing.length > 0) {
        throw new Error(
            `${package_.filename} is missing ${missing.join(", ")}; the newest packages must include graphical software-manager metadata`,
        );
    }
}

function command(executable, arguments_, options = {}) {
    return new Promise((resolveCommand, reject) => {
        const child = spawn(executable, arguments_, { ...options, stdio: ["ignore", "pipe", "inherit"] });
        let stdout = "";
        child.stdout.setEncoding("utf8");
        child.stdout.on("data", (chunk) => { stdout += chunk; });
        child.on("error", reject);
        child.on("close", (code) => code === 0 ? resolveCommand(stdout) : reject(new Error(`${executable} exited with ${code}`)));
    });
}
