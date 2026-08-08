import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { parseBinaryPathArgument } from "./verify-release-build-args.mjs";
import { findForbiddenFrontendMarkers } from "./verify-release-build-core.mjs";

test("accepts a binary path forwarded after pnpm's argument separator", () => {
  assert.equal(
    parseBinaryPathArgument(["--", "target/release/Grayslate"]),
    "target/release/Grayslate",
  );
  assert.equal(
    parseBinaryPathArgument(["target/release/Grayslate"]),
    "target/release/Grayslate",
  );
});

test("rejects ambiguous release binary paths", () => {
  assert.throws(
    () => parseBinaryPathArgument(["first", "second"]),
    /Expected at most one binary path/,
  );
});

test("detects frontend E2E markers in uncompressed build output", (context) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "grayslate-release-check-"));
  context.after(() => fs.rmSync(directory, { recursive: true, force: true }));

  fs.mkdirSync(path.join(directory, "_app", "immutable"), { recursive: true });
  fs.writeFileSync(
    path.join(directory, "_app", "immutable", "app.js"),
    "window.__grayslateE2E = {}; const id = 'grayslate-e2e-determinism';",
    "utf8",
  );

  assert.deepEqual(findForbiddenFrontendMarkers(directory), [
    "__grayslateE2E",
    "grayslate-e2e-determinism",
  ]);
});

test("accepts ordinary production frontend output", (context) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "grayslate-release-check-"));
  context.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  fs.writeFileSync(path.join(directory, "app.js"), "console.log('production');", "utf8");

  assert.deepEqual(findForbiddenFrontendMarkers(directory), []);
});
