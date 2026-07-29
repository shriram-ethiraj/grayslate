import assert from "node:assert/strict";
import test from "node:test";
import { parseBinaryPathArgument } from "./verify-release-build-args.mjs";

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
