import assert from "node:assert/strict";
import {
    cp,
    mkdtemp,
    mkdir,
    readFile,
    readdir,
    rm,
    writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";

const version = "1.2.3";
let moduleRun = 0;

async function fixture(root, relativePath, contents = "artifact") {
    const path = join(root, relativePath);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, contents);
}

async function run(script, arguments_) {
    const originalArguments = process.argv;
    process.argv = [process.execPath, script, ...arguments_];
    try {
        moduleRun += 1;
        await import(new URL(`${script}?test-run=${moduleRun}`, import.meta.url));
    } finally {
        process.argv = originalArguments;
    }
}

test("release scripts stage and assemble every updater platform", async () => {
    const root = await mkdtemp(join(tmpdir(), "grayslate-release-test-"));
    try {
        const definitions = [
            {
                target: "macos-universal",
                files: [
                    ["bundle/dmg/Grayslate.dmg"],
                    ["bundle/macos/Grayslate.app.tar.gz"],
                    ["bundle/macos/Grayslate.app.tar.gz.sig", "mac-signature"],
                ],
            },
            {
                target: "macos-universal-system",
                files: [["bundle/dmg/Grayslate.dmg"]],
            },
            {
                target: "linux-x86_64-direct",
                files: [
                    ["bundle/appimage/Grayslate.AppImage"],
                    ["bundle/appimage/Grayslate.AppImage.sig", "linux-signature"],
                ],
            },
            {
                target: "linux-x86_64-system",
                files: [
                    ["bundle/deb/grayslate.deb"],
                    ["bundle/rpm/grayslate.rpm"],
                ],
            },
            {
                target: "windows-x86_64",
                files: [
                    ["bundle/nsis/Grayslate-setup.exe"],
                    ["bundle/nsis/Grayslate-setup.exe.sig", "windows-x64-signature"],
                ],
            },
            {
                target: "windows-aarch64",
                files: [
                    ["bundle/nsis/Grayslate-setup.exe"],
                    ["bundle/nsis/Grayslate-setup.exe.sig", "windows-arm-signature"],
                ],
            },
        ];

        const assets = join(root, "assets");
        await mkdir(assets);
        for (const definition of definitions) {
            const input = join(root, "inputs", definition.target);
            const output = join(root, "outputs", definition.target);
            for (const [path, contents] of definition.files) {
                await fixture(input, path, contents);
            }
            await run("stage-artifacts.mjs", [
                "--target",
                definition.target,
                "--version",
                version,
                "--bundle-root",
                input,
                "--output",
                output,
            ]);
            for (const filename of await readdir(output)) {
                await cp(join(output, filename), join(assets, filename));
            }
        }

        await writeFile(join(assets, `Grayslate-${version}-source.tar.gz`), "source");
        await run("render-package-templates.mjs", [
            "--version",
            version,
            "--assets",
            assets,
            "--output",
            assets,
        ]);
        await run("assemble-release.mjs", [
            "--version",
            version,
            "--directory",
            assets,
            "--repository",
            "example/grayslate",
            "--published-at",
            "2026-01-02T03:04:05Z",
        ]);

        const latest = JSON.parse(await readFile(join(assets, "latest.json"), "utf8"));
        assert.equal(latest.version, version);
        assert.deepEqual(Object.keys(latest.platforms).sort(), [
            "linux-x86_64",
            "macos-universal",
            "windows-aarch64",
            "windows-x86_64",
        ]);
        assert.match(
            latest.platforms["windows-aarch64"].url,
            /windows-aarch64-setup\.exe$/u,
        );

        const aliases = new Map([
            [
                `Grayslate-${version}-macos-universal.dmg`,
                "grayslate-macos-universal.dmg",
            ],
            [
                `Grayslate-${version}-windows-x86_64-setup.exe`,
                "grayslate-windows-x86_64-setup.exe",
            ],
            [
                `Grayslate-${version}-windows-aarch64-setup.exe`,
                "grayslate-windows-aarch64-setup.exe",
            ],
            [
                `Grayslate-${version}-linux-x86_64.AppImage`,
                "grayslate-linux-x86_64.AppImage",
            ],
            [`Grayslate-${version}-linux-x86_64.deb`, "grayslate-linux-x86_64.deb"],
            [`Grayslate-${version}-linux-x86_64.rpm`, "grayslate-linux-x86_64.rpm"],
        ]);

        for (const [source, alias] of aliases) {
            assert.deepEqual(await readFile(join(assets, alias)), await readFile(join(assets, source)));
        }

        const checksums = await readFile(join(assets, "SHA256SUMS"), "utf8");
        assert.match(checksums, /latest\.json/u);
        assert.match(checksums, /macos-universal-homebrew\.dmg/u);
        for (const alias of aliases.values()) {
            assert.match(checksums, new RegExp(`  ${alias.replaceAll(".", "\\.")}$`, "mu"));
        }
        assert.ok((await readFile(join(assets, "grayslate.rb"), "utf8")).includes(version));
    } finally {
        await rm(root, { recursive: true, force: true });
    }
});

test("set-version updates every release-owned version field", async () => {
    const root = await mkdtemp(join(tmpdir(), "grayslate-version-test-"));
    try {
        await fixture(root, "package.json", '{\n  "version": "0.1.0",\n}\n');
        await fixture(
            root,
            "src-tauri/tauri.conf.json",
            '{\n  "version": "0.1.0",\n}\n',
        );
        await fixture(root, "src-tauri/Cargo.toml", '[package]\nversion = "0.1.0"\n');
        await fixture(
            root,
            "Cargo.lock",
            '[[package]]\nname = "Grayslate"\nversion = "0.1.0"\n',
        );
        await fixture(
            root,
            "packaging/linux/app.grayslate.Grayslate.metainfo.xml",
            "<releases>\n    <release version=\"0.1.0\" date=\"2026-01-01\" />\n</releases>\n",
        );

        await run("set-version.mjs", [
            "0.1.2",
            "--root",
            root,
            "--date",
            "2026-07-17",
        ]);

        assert.match(await readFile(join(root, "package.json"), "utf8"), /"version": "0\.1\.2"/u);
        assert.match(
            await readFile(join(root, "src-tauri/tauri.conf.json"), "utf8"),
            /"version": "0\.1\.2"/u,
        );
        assert.match(
            await readFile(join(root, "src-tauri/Cargo.toml"), "utf8"),
            /version = "0\.1\.2"/u,
        );
        assert.match(
            await readFile(join(root, "Cargo.lock"), "utf8"),
            /name = "Grayslate"\nversion = "0\.1\.2"/u,
        );
        assert.match(
            await readFile(
                join(root, "packaging/linux/app.grayslate.Grayslate.metainfo.xml"),
                "utf8",
            ),
            /<release version="0\.1\.2" date="2026-07-17" \/>/u,
        );
    } finally {
        await rm(root, { recursive: true, force: true });
    }
});
