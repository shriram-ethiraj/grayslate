import { spawn } from "node:child_process";
import { resolve } from "node:path";
import { cacheControlFor, contentTypeFor, objectKey, parseArguments, walkFiles } from "./lib.mjs";

const args = parseArguments(process.argv.slice(2));
const root = resolve(args.get("directory") ?? "package-repository");
const bucket = args.get("bucket") ?? process.env.GRAYSLATE_R2_BUCKET;
if (!bucket || !/^[a-z0-9][a-z0-9-]*[a-z0-9]$/u.test(bucket)) {
    throw new Error("Pass a valid --bucket or set GRAYSLATE_R2_BUCKET");
}

const files = await walkFiles(root);
files.sort((left, right) => uploadRank(objectKey(root, left)) - uploadRank(objectKey(root, right)) || left.localeCompare(right));
for (const path of files) {
    const key = objectKey(root, path);
    await wrangler([
        "r2", "object", "put", `${bucket}/${key}`, "--remote", "--force", "--file", path,
        "--content-type", contentTypeFor(key), "--cache-control", cacheControlFor(key),
    ]);
}

function uploadRank(key) {
    if (key.endsWith("/InRelease") || key.endsWith("/repomd.xml")) return 3;
    if (key.endsWith("/Release") || key.endsWith("/Release.gpg") || key.endsWith("/repomd.xml.asc")) return 2;
    if (key.includes("/pool/") || key.endsWith(".rpm") || key.includes("/by-hash/") || key.includes("/repodata/")) return 0;
    return 1;
}

function wrangler(arguments_) {
    return new Promise((resolveCommand, reject) => {
        const child = spawn("wrangler", arguments_, { stdio: "inherit" });
        child.on("error", reject);
        child.on("close", (code) => code === 0 ? resolveCommand() : reject(new Error(`wrangler exited with ${code}`)));
    });
}
