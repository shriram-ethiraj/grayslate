import { spawn } from "node:child_process";
import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { parseArguments } from "./lib.mjs";

const args = parseArguments(process.argv.slice(2));
const output = resolve(args.get("output") ?? "repository-packages");
const repository = args.get("repository") ?? process.env.GITHUB_REPOSITORY;
if (!repository) throw new Error("Pass --repository or set GITHUB_REPOSITORY");
await mkdir(output, { recursive: true });

const releases = JSON.parse(await gh([
    "release", "list", "--repo", repository, "--limit", "1000",
    "--exclude-drafts", "--exclude-pre-releases", "--json", "tagName",
]));
for (const { tagName } of releases) {
    if (!/^v\d+\.\d+\.\d+$/u.test(tagName)) continue;
    const version = tagName.slice(1);
    await gh([
        "release", "download", tagName, "--repo", repository, "--dir", output,
        "--pattern", `Grayslate-${version}-linux-x86_64.deb`,
        "--pattern", `Grayslate-${version}-linux-x86_64.rpm`,
        "--skip-existing",
    ]);
}

function gh(arguments_) {
    return new Promise((resolveCommand, reject) => {
        const child = spawn("gh", arguments_, { stdio: ["ignore", "pipe", "inherit"] });
        let stdout = "";
        child.stdout.setEncoding("utf8");
        child.stdout.on("data", (chunk) => { stdout += chunk; });
        child.on("error", reject);
        child.on("close", (code) => code === 0 ? resolveCommand(stdout) : reject(new Error(`gh exited with ${code}`)));
    });
}

