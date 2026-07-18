import { createHash } from "node:crypto";
import { readdir } from "node:fs/promises";
import { join, relative, sep } from "node:path";

export const REQUIRED_PACKAGE_METADATA_PATHS = Object.freeze([
    "/usr/share/applications/app.grayslate.Grayslate.desktop",
    "/usr/share/metainfo/app.grayslate.Grayslate.metainfo.xml",
    "/usr/share/icons/hicolor/512x512/apps/app.grayslate.Grayslate.png",
]);

export function parseArguments(arguments_) {
    const values = new Map();
    for (let index = 0; index < arguments_.length; index += 2) {
        const key = arguments_[index];
        const value = arguments_[index + 1];
        if (!key?.startsWith("--") || value === undefined) {
            throw new Error(`Invalid argument near ${key ?? "end of input"}`);
        }
        values.set(key.slice(2), value);
    }
    return values;
}

export async function walkFiles(root) {
    const files = [];
    async function walk(directory) {
        for (const entry of await readdir(directory, { withFileTypes: true })) {
            const path = join(directory, entry.name);
            if (entry.isDirectory()) {
                await walk(path);
            } else if (entry.isFile()) {
                files.push(path);
            }
        }
    }
    await walk(root);
    return files.sort();
}

export function objectKey(root, path) {
    return relative(root, path).split(sep).join("/");
}

export function cacheControlFor(key) {
    if (
        key.includes("/pool/") ||
        key.includes("/by-hash/") ||
        /\/repodata\/[0-9a-f]{64}-/u.test(key)
    ) {
        return "public, max-age=31536000, immutable";
    }
    // Historical GitHub RPMs are signed while rebuilding the repository, so
    // their bytes can change even when their versioned object key does not.
    // Force revalidation to keep the package aligned with RPM-MD checksums.
    if (key.endsWith(".rpm")) return "no-cache";
    return "public, max-age=300, must-revalidate";
}

export function contentTypeFor(key) {
    if (key.endsWith(".asc")) return "application/pgp-keys";
    if (key.endsWith(".gpg")) return "application/pgp-keys";
    if (key.endsWith(".deb")) return "application/vnd.debian.binary-package";
    if (key.endsWith(".rpm")) return "application/x-rpm";
    if (key.endsWith(".gz")) return "application/gzip";
    if (key.endsWith(".xz")) return "application/x-xz";
    if (key.endsWith(".xml")) return "application/xml";
    if (key.endsWith(".repo") || key.endsWith(".sources")) return "text/plain; charset=utf-8";
    if (key.endsWith(".sh")) return "text/x-shellscript; charset=utf-8";
    return "application/octet-stream";
}

export function packageListingPaths(contents) {
    const paths = new Set();
    for (const line of contents.split(/\r?\n/u)) {
        const trimmed = line.trim();
        if (!trimmed) continue;

        // rpm -qlp prints only the path. dpkg-deb --contents prefixes it with
        // mode, owner, size, and timestamp columns. The metadata filenames do
        // not contain spaces, so the final column is the archive path in both
        // the GNU and BSD tar listing formats used by dpkg-deb.
        const rawPath = trimmed.startsWith("/") || trimmed.startsWith("./")
            ? trimmed
            : trimmed.split(/\s+/u).at(-1);
        if (!rawPath) continue;

        const relativePath = rawPath.replace(/^\.\//u, "").replace(/^\/+/, "");
        paths.add(`/${relativePath}`);
    }
    return paths;
}

export function missingRequiredPackageMetadata(contents) {
    const paths = packageListingPaths(contents);
    return REQUIRED_PACKAGE_METADATA_PATHS.filter((path) => !paths.has(path));
}

export function newestStablePackage(packages) {
    if (packages.length === 0) throw new Error("Cannot select the newest package from an empty list");
    return packages.reduce((newest, candidate) => (
        compareStableVersions(candidate.version, newest.version) > 0 ? candidate : newest
    ));
}

export function compareStableVersions(left, right) {
    const leftParts = parseStableVersion(left);
    const rightParts = parseStableVersion(right);
    for (let index = 0; index < leftParts.length; index += 1) {
        const difference = leftParts[index] - rightParts[index];
        if (difference !== 0) return difference;
    }
    return 0;
}

function parseStableVersion(version) {
    const match = /^(\d+)\.(\d+)\.(\d+)$/u.exec(version);
    if (!match) {
        throw new Error(`Package version ${version} is not a stable semantic version`);
    }
    return match.slice(1).map(Number);
}

export async function sha256(path) {
    const { readFile } = await import("node:fs/promises");
    return createHash("sha256").update(await readFile(path)).digest("hex");
}
