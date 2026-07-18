import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import { promisify } from "node:util";
import test from "node:test";
import {
    cacheControlFor,
    compareStableVersions,
    contentTypeFor,
    missingRequiredPackageMetadata,
    newestStablePackage,
    objectKey,
    packageListingPaths,
    parseArguments,
    REQUIRED_PACKAGE_METADATA_PATHS,
} from "./lib.mjs";

const execFileAsync = promisify(execFile);
const installerUrl = new URL("../../packaging/repository/install.sh", import.meta.url);

test("repository argument parsing rejects incomplete pairs", () => {
    assert.deepEqual([...parseArguments(["--output", "dist"])], [["output", "dist"]]);
    assert.throws(() => parseArguments(["--output"]));
});

test("R2 metadata distinguishes immutable payloads and live indexes", () => {
    assert.match(cacheControlFor("apt/pool/main/g/grayslate/a.deb"), /immutable/u);
    assert.equal(cacheControlFor("rpm/stable/x86_64/grayslate-1.0.0-1.x86_64.rpm"), "no-cache");
    assert.doesNotMatch(cacheControlFor("apt/dists/stable/InRelease"), /immutable/u);
    assert.equal(contentTypeFor("config/grayslate.sources"), "text/plain; charset=utf-8");
    assert.equal(contentTypeFor("install.sh"), "text/x-shellscript; charset=utf-8");
    assert.equal(contentTypeFor("rpm/stable/x86_64/a.rpm"), "application/x-rpm");
    assert.equal(objectKey("/tmp/out", "/tmp/out/apt/Release"), "apt/Release");
});

test("Linux installer is valid POSIX shell with signed-repository safeguards", async () => {
    await execFileAsync("sh", ["-n", installerUrl.pathname]);
    const installer = await readFile(installerUrl, "utf8");
    assert.match(installer, /grep -Fqx "gpgcheck=1"/u);
    assert.match(installer, /grep -Fqx "repo_gpgcheck=1"/u);
    assert.match(installer, /verify_sha256 "\$GRAYSLATE_APT_KEY_SHA256"/u);
    assert.match(installer, /verify_sha256 "\$GRAYSLATE_RPM_KEY_SHA256"/u);
    assert.match(installer, /main "\$@"\s*$/u);
});

test("package listings normalize DEB, RPM, and dot-relative paths", () => {
    const listing = [
        "-rw-r--r-- 0/0 344 2026-07-18 12:00 usr/share/applications/app.grayslate.Grayslate.desktop",
        "/usr/share/metainfo/app.grayslate.Grayslate.metainfo.xml",
        "./usr/share/icons/hicolor/512x512/apps/app.grayslate.Grayslate.png",
    ].join("\n");

    assert.deepEqual([...packageListingPaths(listing)], REQUIRED_PACKAGE_METADATA_PATHS);
    assert.deepEqual(missingRequiredPackageMetadata(listing), []);
});

test("only the newest package needs the current graphical metadata during backfill", () => {
    const historical = { version: "0.1.4", contents: "/usr/bin/Grayslate" };
    const current = { version: "0.1.5", contents: REQUIRED_PACKAGE_METADATA_PATHS.join("\n") };

    assert.equal(newestStablePackage([current, historical]), current);
    assert.equal(missingRequiredPackageMetadata(historical.contents).length, REQUIRED_PACKAGE_METADATA_PATHS.length);
    assert.deepEqual(missingRequiredPackageMetadata(current.contents), []);
    assert.ok(compareStableVersions("0.10.0", "0.9.9") > 0);
    assert.throws(() => compareStableVersions("0.1.5-beta.1", "0.1.4"));
});
