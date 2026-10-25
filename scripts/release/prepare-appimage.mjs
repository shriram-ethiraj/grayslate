import { createHash } from "node:crypto";
import { chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const launcherUrl = "https://github.com/tauri-apps/binary-releases/releases/download/apprun-old/AppRun-x86_64";
const launcherSha256 = "f30140a43a0a59e46db21bdefdf749b9e9f2c6946e92afabbacf98b8ae73fb4f";

async function downloadLauncher() {
    const response = await fetch(launcherUrl, { signal: AbortSignal.timeout(60_000) });
    if (!response.ok) throw new Error(`AppRun download failed: HTTP ${response.status}`);
    return Buffer.from(await response.arrayBuffer());
}

export async function prepareAppRun(toolsDirectory, { download = downloadLauncher, sha256 = launcherSha256 } = {}) {
    const launcher = join(toolsDirectory, "AppRun-x86_64");
    let data;
    try {
        data = await readFile(launcher);
    } catch (error) {
        if (error.code !== "ENOENT") throw error;
    }
    const missing = data === undefined;
    data ??= await download();
    if (createHash("sha256").update(data).digest("hex") !== sha256) {
        throw new Error("Unexpected Tauri AppRun checksum; review the launcher before packaging.");
    }
    await mkdir(toolsDirectory, { recursive: true });
    if (missing) await writeFile(launcher, data, { mode: 0o755 });
    // Tauri 2.11.4 otherwise downloads AppRun as 0770 and copies that mode into
    // AppRun.wrapped. SquashFS assigns root ownership, denying ordinary users.
    await chmod(launcher, 0o755);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    if (process.platform !== "linux") throw new Error("AppImage packaging requires Linux.");
    const arch = process.env.TAURI_ENV_ARCH ?? process.arch;
    if (arch !== "x86_64" && arch !== "x64") {
        throw new Error(`AppImage launcher preparation does not support ${arch}.`);
    }
    // Match dirs::cache_dir() used by Tauri on Linux.
    const xdgCache = process.env.XDG_CACHE_HOME;
    const cacheDirectory = xdgCache && isAbsolute(xdgCache) ? xdgCache : join(homedir(), ".cache");
    await prepareAppRun(join(cacheDirectory, "tauri"));
    console.log("Prepared AppImage launcher with permissions 0755.");
}
