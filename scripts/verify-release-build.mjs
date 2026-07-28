#!/usr/bin/env node
/**
 * Verify a packaged binary carries no test-only surface.
 *
 * The e2e shims run the *real* authorization and grant paths. That is what
 * makes them useful for testing and exactly what makes them dangerous to ship:
 * anything able to reach the webview could open or claim write access to an
 * arbitrary path.
 *
 * They are gated three ways in the source — `#[cfg(feature = "e2e")]` in
 * `src-tauri/src/lib.rs`, the ACL generated only when `CARGO_FEATURE_E2E` is
 * set in `build.rs`, and the extra capability file. This script checks the
 * built artifact rather than trusting the gates, because a build-configuration
 * mistake is precisely the failure the gates cannot catch.
 *
 * The forbidden list is *derived* from `E2E_COMMANDS` in
 * `src-tauri/src/command_names.rs` rather than copied here. A hardcoded list
 * silently stops covering the commands added after it was written: this script
 * checked two names while four existed, so the queued-dialog commands could
 * have shipped and the check would still have printed OK.
 *
 * Usage: node scripts/verify-release-build.mjs [path-to-binary]
 */
import fs from "node:fs";
import path from "node:path";

const COMMAND_NAMES_PATH = path.resolve(
  import.meta.dirname,
  "..",
  "src-tauri",
  "src",
  "command_names.rs",
);

/** The `E2E_COMMANDS` slice, read from source so it cannot drift. */
function forbiddenCommands() {
  const source = fs.readFileSync(COMMAND_NAMES_PATH, "utf8");
  const slice = /pub const E2E_COMMANDS:\s*&\[&str\]\s*=\s*&\[([^\]]*)\]/.exec(source);
  if (!slice) {
    console.error(
      `Could not find E2E_COMMANDS in ${COMMAND_NAMES_PATH}. This check cannot ` +
        "be allowed to pass by default — fix the parser or the declaration.",
    );
    process.exit(2);
  }
  const names = [...slice[1].matchAll(/"([a-z0-9_]+)"/g)].map((match) => match[1]);
  if (names.length === 0) {
    console.error("E2E_COMMANDS parsed as empty; refusing to report a vacuous pass.");
    process.exit(2);
  }
  return names;
}

const FORBIDDEN = forbiddenCommands();

const binaryPath =
  process.argv[2] ??
  path.resolve(
    process.cwd(),
    "target/release",
    process.platform === "win32" ? "Grayslate.exe" : "Grayslate",
  );

if (!fs.existsSync(binaryPath)) {
  console.error(`No binary at ${binaryPath}. Build a release first.`);
  process.exit(2);
}

const contents = fs.readFileSync(binaryPath);
const found = FORBIDDEN.filter((symbol) => contents.includes(symbol));

if (found.length > 0) {
  console.error(
    `\nRELEASE BLOCKED: ${binaryPath} contains test-only command name(s):\n` +
      found.map((symbol) => `  - ${symbol}`).join("\n") +
      "\n\nThis binary was built with --features e2e. A distributed build must " +
      "never set that feature (see src-tauri/Cargo.toml).\n",
  );
  process.exit(1);
}

console.log(`OK: ${path.basename(binaryPath)} contains none of: ${FORBIDDEN.join(", ")}`);
process.exit(0);
