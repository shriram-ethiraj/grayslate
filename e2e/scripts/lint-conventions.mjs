#!/usr/bin/env node
/**
 * Convention checks for the E2E suite.
 *
 * The repository has no linter (only `svelte-check` and `tsc`), so rather than
 * pulling in ESLint for three rules, these are enforced here. Each rule exists
 * because the audited suite broke it in a way that produced tests which passed
 * for the wrong reason.
 *
 * Run: pnpm run e2e:lint
 */
import fs from "node:fs";
import path from "node:path";
import ts from "typescript";

const REPO_ROOT = path.resolve(import.meta.dirname, "..", "..");
const E2E_ROOT = path.join(REPO_ROOT, "e2e");
const SPECS_ROOT = path.join(E2E_ROOT, "specs");
const DRIVER_ROOT = path.join(E2E_ROOT, "driver");

/**
 * Files still awaiting migration to the driver/page-object layer.
 *
 * This list may only shrink. Every entry is a spec that predates the harness
 * rebuild; Phase 2 empties it. A new spec must never be added here.
 */
const MIGRATION_BACKLOG = new Set([
]);

/** IPC is legitimate here: the shims, the security threat model, the driver. */
const IPC_ALLOWED_PREFIXES = ["driver/", "pages/", "fixtures/", "specs/security/"];

const failures = [];

function fail(file, line, rule, detail) {
  failures.push({ file, line, rule, detail });
}

function walk(directory, predicate) {
  if (!fs.existsSync(directory)) return [];
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) return walk(absolute, predicate);
    return entry.isFile() && predicate(entry.name) ? [absolute] : [];
  });
}

const relative = (absolute) => path.relative(E2E_ROOT, absolute).split(path.sep).join("/");
const lineOf = (source, index) => source.slice(0, index).split("\n").length;

/**
 * Blank out comments while preserving every byte offset and line break.
 *
 * These rules are about code, and the rule descriptions themselves quote the
 * banned patterns (`browser.pause`, bare `waitUntil`) in doc comments. Scanning
 * raw text would flag the documentation explaining the rule.
 */
function stripComments(source) {
  const out = source.split("");
  let index = 0;
  let quote = null;

  const blank = (from, to) => {
    for (let i = from; i < to && i < out.length; i += 1) {
      if (out[i] !== "\n") out[i] = " ";
    }
  };

  while (index < source.length) {
    const character = source[index];

    if (quote) {
      if (character === "\\") index += 2;
      else {
        if (character === quote) quote = null;
        index += 1;
      }
      continue;
    }

    if (character === '"' || character === "'" || character === "`") {
      quote = character;
      index += 1;
      continue;
    }

    if (character === "/" && source[index + 1] === "/") {
      const end = source.indexOf("\n", index);
      const stop = end === -1 ? source.length : end;
      blank(index, stop);
      index = stop;
      continue;
    }

    if (character === "/" && source[index + 1] === "*") {
      const end = source.indexOf("*/", index + 2);
      const stop = end === -1 ? source.length : end + 2;
      blank(index, stop);
      index = stop;
      continue;
    }

    index += 1;
  }

  return out.join("");
}

// ── R3: no fixed sleeps ────────────────────────────────────────────────────
//
// Every `browser.pause` in the audited suite guarded a *negative* assertion,
// which is the one place a sleep is actively harmful: on a slower machine the
// violation simply had not happened yet, so the test passed for the wrong
// reason. Use `expectSettledAbsent` from `e2e/assertions/matchers.ts`.
function checkNoFixedSleeps(file, source) {
  const key = relative(file);
  if (MIGRATION_BACKLOG.has(key)) return;
  for (const match of source.matchAll(/browser\s*\.\s*pause\s*\(/g)) {
    fail(
      key,
      lineOf(source, match.index),
      "no-fixed-sleeps",
      "browser.pause() is banned in specs. Use expectSettledAbsent() for negatives, or a waitFor() on the real signal.",
    );
  }
}

// ── R4: every wait carries a timeout and a message ─────────────────────────
//
// A bare wait inherits the global 15 s ceiling and fails with a message that
// says nothing about what was expected. Parse the call structurally: the old
// text check used `args.includes("timeout")`, which was accidentally satisfied
// by the substring in `timeoutMsg`.
function checkWaitsAreExplicit(file, source) {
  const key = relative(file);
  if (MIGRATION_BACKLOG.has(key)) return;
  const sourceFile = ts.createSourceFile(key, source, ts.ScriptTarget.Latest, true);
  const webdriverWaits = new Map([
    ["waitUntil", 1],
    ["waitForDisplayed", 0],
    ["waitForExist", 0],
    ["waitForClickable", 0],
    ["waitForEnabled", 0],
  ]);

  const propertyName = (property) => {
    if (
      ts.isIdentifier(property.name) ||
      ts.isStringLiteral(property.name) ||
      ts.isNumericLiteral(property.name)
    ) {
      return property.name.text;
    }
    return property.name.getText(sourceFile);
  };

  const visit = (node) => {
    if (
      ts.isCallExpression(node) &&
      ts.isPropertyAccessExpression(node.expression)
    ) {
      const method = node.expression.name.text;
      const optionsIndex = webdriverWaits.get(method);
      if (optionsIndex !== undefined) {
        const options = node.arguments[optionsIndex];
        const keys =
          options && ts.isObjectLiteralExpression(options)
            ? new Set(options.properties.map(propertyName))
            : new Set();
        if (!keys.has("timeout") || !keys.has("timeoutMsg")) {
          const line =
            sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1;
          fail(
            key,
            line,
            "explicit-waits",
            `${method}() needs exact timeout and timeoutMsg options. Prefer waitFor() from e2e/driver/wait.ts where possible.`,
          );
        }
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
}

// ── R5: UI flows are driven through the UI ─────────────────────────────────
//
// A test that reaches for IPC instead of clicking proves the backend works and
// nothing about the feature the user touches. Read-only oracles that WebDriver
// genuinely cannot express are allowed when annotated.
function checkIpcIsJustified(file, source, rawSource) {
  const key = relative(file);
  if (MIGRATION_BACKLOG.has(key)) return;
  if (IPC_ALLOWED_PREFIXES.some((prefix) => key.startsWith(prefix))) return;

  const lines = source.split("\n");
  // The justification lives in a comment, so look for it in the raw text.
  const rawLines = rawSource.split("\n");
  lines.forEach((line, index) => {
    if (!/\b(invokeInApp|rawInvoke)\s*\(/.test(line)) return;
    const window = rawLines.slice(Math.max(0, index - 3), index + 1).join("\n");
    if (window.includes("ipc-oracle:")) return;
    fail(
      key,
      index + 1,
      "ui-flows-through-ui",
      "Direct IPC in a spec. Drive the UI, or annotate the read-only oracle with `// ipc-oracle: <why>`.",
    );
  });
}

// ── R-probe: browser.execute belongs in the driver layer ───────────────────
//
// The audited suite used `browser.execute` to synthesize dblclick and
// pointerenter events, set input values directly, assign scrollTop, and inject
// nodes into the app's DOM — bypassing the exact code paths under test.
function checkNoAdHocScripting(file, source, rawSource) {
  const key = relative(file);
  if (MIGRATION_BACKLOG.has(key)) return;
  if (key.startsWith("driver/")) return;

  const rawLines = rawSource.split("\n");
  for (const match of source.matchAll(/browser\s*\.\s*execute(Async)?\s*\(/g)) {
    const line = lineOf(source, match.index);
    // The security specs must be able to introduce hostile content — that is
    // the threat model. The grant is per-site and annotated, not a blanket
    // exemption for the directory, and each probe must clean up in a finally.
    if (key.startsWith("specs/security/")) {
      const window = rawLines.slice(Math.max(0, line - 4), line).join("\n");
      if (window.includes("security-probe:")) continue;
    }
    fail(
      key,
      line,
      "no-ad-hoc-scripting",
      key.startsWith("specs/security/")
        ? "Security probes must be annotated with `// security-probe: <why>` and cleaned up in a finally."
        : "browser.execute belongs in e2e/driver/probe.ts (reads) or e2e/driver/invoke.ts (IPC).",
    );
  }
}

// ── Driver layer must stay read-only ───────────────────────────────────────
function checkProbeDoesNotMutate() {
  const probe = path.join(DRIVER_ROOT, "probe.ts");
  if (!fs.existsSync(probe)) return;
  const source = stripComments(fs.readFileSync(probe, "utf8"));
  const mutations = [
    /dispatchEvent\s*\(/,
    /\.value\s*=\s*/,
    /setAttribute\s*\(/,
    /\.scrollTop\s*=\s*/,
    /appendChild\s*\(/,
    /\.remove\s*\(\s*\)/,
    /\.focus\s*\(\s*\)/,
    /\.click\s*\(\s*\)/,
  ];
  for (const pattern of mutations) {
    const match = source.match(pattern);
    if (!match) continue;
    fail(
      "driver/probe.ts",
      lineOf(source, source.indexOf(match[0])),
      "probe-is-read-only",
      `probe.ts must only read page state; found '${match[0].trim()}'. Move the interaction into a page object that clicks or types.`,
    );
  }
}

// ── The migration backlog may only shrink ──────────────────────────────────
function checkBacklogIsAccurate(files) {
  const present = new Set(files.map(relative));
  for (const entry of MIGRATION_BACKLOG) {
    if (present.has(entry)) continue;
    fail(
      "scripts/lint-conventions.mjs",
      1,
      "stale-backlog",
      `MIGRATION_BACKLOG lists '${entry}', which no longer exists. Remove the entry.`,
    );
  }
}

const sourceFiles = [
  ...walk(SPECS_ROOT, (name) => name.endsWith(".ts")),
  ...walk(path.join(E2E_ROOT, "helpers"), (name) => name.endsWith(".ts")),
  ...walk(path.join(E2E_ROOT, "pages"), (name) => name.endsWith(".ts")),
  ...walk(path.join(E2E_ROOT, "fixtures"), (name) => name.endsWith(".ts")),
  ...walk(DRIVER_ROOT, (name) => name.endsWith(".ts")),
  ...walk(path.join(E2E_ROOT, "assertions"), (name) => name.endsWith(".ts")),
];

for (const file of sourceFiles) {
  const rawSource = fs.readFileSync(file, "utf8");
  const source = stripComments(rawSource);
  checkNoFixedSleeps(file, source);
  checkWaitsAreExplicit(file, source);
  checkIpcIsJustified(file, source, rawSource);
  checkNoAdHocScripting(file, source, rawSource);
}
checkProbeDoesNotMutate();
checkBacklogIsAccurate(sourceFiles);

if (failures.length === 0) {
  const remaining = MIGRATION_BACKLOG.size;
  console.log(
    `e2e conventions: OK (${sourceFiles.length} files checked, ${remaining} awaiting migration).`,
  );
  process.exit(0);
}

console.error(`e2e conventions: ${failures.length} violation(s)\n`);
for (const failure of failures) {
  console.error(`  ${failure.file}:${failure.line}  [${failure.rule}]`);
  console.error(`      ${failure.detail}`);
}
process.exit(1);
