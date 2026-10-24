import { execFileSync } from "node:child_process";
import { chmod, mkdtemp, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export async function verifyAppDir(directory) {
    for (const name of ["AppRun", "AppRun.wrapped", "usr/bin/Grayslate"]) {
        const info = await stat(join(directory, name));
        // Check other-user read/execute bits, regardless of the inspecting user's
        // ownership or root privileges. A builder can execute a broken 0770 file.
        if (!info.isFile() || (info.mode & 0o005) !== 0o005) {
            throw new Error(`${name} must be readable and executable by ordinary users (mode ${(info.mode & 0o777).toString(8)}).`);
        }
    }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    if (!process.argv[2]) throw new Error("Usage: node scripts/release/verify-appimage.mjs <AppImage>");
    const artifact = resolve(process.argv[2]);
    const temporary = await mkdtemp(join(tmpdir(), "grayslate-appimage-check-"));
    try {
        await chmod(artifact, 0o755);
        // Execute only the locally built AppImage runtime, not Grayslate. Inspect
        // the final SquashFS payload, rather than the intermediate build folder.
        execFileSync(artifact, ["--appimage-extract"], {
            cwd: temporary,
            timeout: 60_000,
            stdio: ["ignore", "ignore", "pipe"],
        });
        await verifyAppDir(join(temporary, "squashfs-root"));
        console.log("AppImage launchers and Grayslate are executable by ordinary users.");
    } finally {
        await rm(temporary, { recursive: true, force: true });
    }
}
