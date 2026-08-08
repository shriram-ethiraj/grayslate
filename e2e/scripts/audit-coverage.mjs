#!/usr/bin/env node
/**
 * Coverage audit.
 *
 * Parses every spec with the TypeScript compiler and collects the requirement
 * ids claimed by `scenario(...)` calls, then reconciles them against
 * `e2e/coverage/requirements.ts`.
 *
 * Deliberately *not* a text search. An earlier design counted a behavior as
 * covered when a spec mentioned its `data-testid` or IPC command name, which
 * measures vocabulary rather than verification: a spec can name `action-save`
 * without ever saving, and name `write_file_content` without asserting a byte.
 * Here a claim only counts when it is an executable call whose id resolves to a
 * registry entry.
 *
 * Also reconciles the source-derived catalogs — transformation ids, settings
 * keys, and Tauri commands — so a new one cannot appear without being assigned
 * to E2E, to a lower-level test, or to a reviewed manual deferral.
 *
 * Run: pnpm run e2e:coverage
 */
import fs from "node:fs";
import path from "node:path";
import ts from "typescript";

const REPO_ROOT = path.resolve(import.meta.dirname, "..", "..");
const E2E_ROOT = path.join(REPO_ROOT, "e2e");
const SPECS_ROOT = path.join(E2E_ROOT, "specs");

const STRICT = process.env.E2E_COVERAGE_STRICT === "1" || process.argv.includes("--strict");

// ── Exact evidence maps ────────────────────────────────────────────────────
//
// An aggregate existence check is not evidence. The previous version asked, for
// each `cancel_*` command, whether *any* requirement id contained "cancel" — a
// predicate that does not mention the command, so one requirement satisfied all
// five, and `file.guard.cancel-and-discard` (a dirty-file prompt, not a
// cancellable backend request) satisfied them on its own. `csv_cancel` escaped
// entirely because it does not start with `cancel_`.
//
// Each cancellable backend request is therefore mapped to the exact behavior
// that proves it. A new cancellation command must be added here or the audit
// fails: the map is reconciled against the command catalog in both directions.
const CANCELLATION_EVIDENCE = {
  cancel_file_read: "file.read.cancel",
  cancel_editor_find: "editor.find.cancel",
  cancel_transformation: "transform.large.cancel",
  cancel_markdown_preview: "markdown.cancel",
  cancel_sidebar_search: "sidebar.search.cancel",
  csv_cancel: "csv.cancel",
};

// Commands that cancel work but are not named `cancel_*`. Kept explicit so the
// catalog sweep below can find every cancellable request regardless of naming.
const EXTRA_CANCELLATION_COMMANDS = ["csv_cancel"];

// Every persisted setting must have a behavior proving the value survives the
// round trip a user cares about. `lastActiveFile` is internal bookkeeping with
// no Settings UI, so it is proven by the startup behavior that reads it.
const SETTINGS_EVIDENCE = {
  theme: "shell.theme.persist-across-restart",
  fontSize: "editor.font-size",
  wordWrap: "shell.settings.persist-across-restart",
  sidebarWidth: "sidebar.width.persist-across-restart",
  sidebarOpen: "sidebar.open.persist-across-restart",
  startupBehavior: "file.restart.reopens-last",
  lastActiveFile: "file.restart.reopens-last",
  defaultIndentMode: "shell.settings.indent-default",
  defaultIndentSize: "shell.settings.indent-default",
  confirmBeforeDelete: "file.delete.without-confirmation",
  automaticUpdateChecks: "shell.settings.persist-across-restart",
  defaultLineEnding: "format.eol.default-for-new",
  defaultEncoding: "format.encoding.default-for-new",
};

// The exact set of behaviors known to be unwritten, as of the last audit.
//
// A count-based ratchet is not enough: removing one gap while adding another
// keeps the count flat and hides the regression. This list is compared by
// identity, so a *new* unclaimed id fails the audit even while the backlog is
// non-empty. Delete entries as they are covered; the list must be empty before
// strict mode is turned on.
const KNOWN_GAPS = new Set([
]);

/**
 * Which CI job owns a behavior.
 *
 * Derived from `area` rather than stored per entry, so it cannot drift from the
 * registry. The tier matters because the jobs differ in whether they block: a
 * functional behavior claimed from `specs/visual/` would silently become
 * non-blocking the moment the visual job is made advisory.
 */
function tierOf(requirement) {
  if (requirement.area === "visual") return "visual";
  if (requirement.area === "security") return "security";
  return "functional";
}

function tierOfSpec(relativePath) {
  if (relativePath.startsWith(`specs${path.sep}visual${path.sep}`)) return "visual";
  if (relativePath.startsWith(`specs${path.sep}security${path.sep}`)) return "security";
  return "functional";
}

// ── Load the registry ──────────────────────────────────────────────────────
//
// Read it as source rather than importing: the module is TypeScript, and this
// script must run under plain node with no build step.
function loadRequirements() {
  const source = fs.readFileSync(path.join(E2E_ROOT, "coverage", "requirements.ts"), "utf8");
  const file = ts.createSourceFile("requirements.ts", source, ts.ScriptTarget.Latest, true);
  const requirements = [];

  const visit = (node) => {
    if (ts.isObjectLiteralExpression(node)) {
      const entry = {};
      for (const property of node.properties) {
        if (!ts.isPropertyAssignment(property)) continue;
        const key = property.name.getText(file).replace(/['"]/g, "");
        const value = property.initializer;
        if (ts.isStringLiteral(value) || ts.isNoSubstitutionTemplateLiteral(value)) {
          entry[key] = value.text;
        }
      }
      if (entry.id && entry.coverage) requirements.push(entry);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);

  return requirements;
}

// ── Collect scenario() claims from the specs ───────────────────────────────
function walk(directory, predicate) {
  if (!fs.existsSync(directory)) return [];
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) return walk(absolute, predicate);
    return entry.isFile() && predicate(entry.name) ? [absolute] : [];
  });
}

function collectClaims() {
  const claims = [];
  for (const specPath of walk(SPECS_ROOT, (name) => name.endsWith(".e2e.ts"))) {
    const source = fs.readFileSync(specPath, "utf8");
    const file = ts.createSourceFile(specPath, source, ts.ScriptTarget.Latest, true);

    const visit = (node) => {
      if (
        ts.isCallExpression(node) &&
        ts.isIdentifier(node.expression) &&
        node.expression.text === "scenario"
      ) {
        const [idArgument, , bodyArgument] = node.arguments;
        const line = file.getLineAndCharacterOfPosition(node.getStart(file)).line + 1;
        const spec = path.relative(E2E_ROOT, specPath);
        const where = `${spec}:${line}`;

        if (!idArgument || !ts.isStringLiteral(idArgument)) {
          claims.push({ id: null, spec, where, executable: false });
        } else {
          // A claim only counts when it carries a real test body.
          const executable =
            Boolean(bodyArgument) &&
            (ts.isArrowFunction(bodyArgument) || ts.isFunctionExpression(bodyArgument));
          claims.push({ id: idArgument.text, spec, where, executable });
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(file);
  }
  return claims;
}

// ── Source-derived catalogs ────────────────────────────────────────────────
function transformationIds() {
  const source = fs.readFileSync(
    path.join(REPO_ROOT, "src", "lib", "transformations", "actions.ts"),
    "utf8",
  );
  const file = ts.createSourceFile("actions.ts", source, ts.ScriptTarget.Latest, true);
  const ids = new Set();
  const visit = (node) => {
    if (ts.isPropertyAssignment(node) && node.name.getText(file) === "id") {
      const value = node.initializer;
      if (ts.isStringLiteral(value)) ids.add(value.text);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  return [...ids];
}

/** Rust-side transformation ids declared by serde rename attributes. */
function rustTransformationIds() {
  const source = fs.readFileSync(
    path.join(REPO_ROOT, "src-tauri", "src", "commands", "transform.rs"),
    "utf8",
  );
  const enumStart = source.indexOf("enum TransformationActionId");
  const enumEnd = source.indexOf("\n}", enumStart);
  if (enumStart === -1 || enumEnd === -1) return [];
  const body = source.slice(enumStart, enumEnd);
  return [...body.matchAll(/#\[serde\(rename\s*=\s*"([^"]+)"\)\]/g)].map(
    (match) => match[1],
  );
}

/**
 * The persisted settings schema.
 *
 * Read from the `AppSettings` interface rather than `DEFAULT_SETTINGS`, because
 * a key can exist in the type before it has a default and that is exactly when
 * it is easiest to ship untested.
 */
function settingKeys() {
  const source = fs.readFileSync(
    path.join(REPO_ROOT, "src", "lib", "state", "appSettings.svelte.ts"),
    "utf8",
  );
  const file = ts.createSourceFile("appSettings.svelte.ts", source, ts.ScriptTarget.Latest, true);
  const keys = [];
  const visit = (node) => {
    if (ts.isInterfaceDeclaration(node) && node.name.text === "AppSettings") {
      for (const member of node.members) {
        if (ts.isPropertySignature(member) && member.name) {
          keys.push(member.name.getText(file).replace(/['"]/g, ""));
        }
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  return keys;
}

/**
 * Shortcut catalog vs. real registrations.
 *
 * `src/lib/shortcuts.ts` is documentation consumed by the help dialog and the
 * tooltips; the actual bindings are registered separately across Titlebar,
 * +layout, the sidebar, and the CodeMirror keymap. Nothing links the two, so
 * they drift silently — a shortcut can work while being undocumented, or be
 * documented after it stops working. Neither is observable at runtime.
 */
function shortcutDrift() {
  const catalogPath = path.join(REPO_ROOT, "src", "lib", "shortcuts.ts");
  if (!fs.existsSync(catalogPath)) return [];

  const source = fs.readFileSync(catalogPath, "utf8");
  const file = ts.createSourceFile("shortcuts.ts", source, ts.ScriptTarget.Latest, true);

  const documented = new Set();
  const visit = (node) => {
    if (ts.isPropertyAssignment(node) && node.name.getText(file) === "keys") {
      const value = node.initializer;
      if (ts.isStringLiteral(value)) documented.add(value.text);
      else if (ts.isArrayLiteralExpression(value)) {
        for (const element of value.elements) {
          if (ts.isStringLiteral(element)) documented.add(element.text);
        }
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(file);

  // Collect every key string that actually reaches a binding.
  //
  // Scanning for `key:` across whole files does not work: it is far too common a
  // property name, and matches list keys (`key: "all"`), element keys
  // (`key: \`untitled:${now}\``), and anything else spelled the same. Every
  // extraction below is therefore scoped to the argument span of a call that
  // really does register something.
  const registered = new Set();
  const searchRoots = [
    path.join(REPO_ROOT, "src", "lib"),
    path.join(REPO_ROOT, "src", "routes"),
  ];
  const sources = searchRoots.flatMap((root) =>
    walk(root, (name) => name.endsWith(".ts") || name.endsWith(".svelte")),
  );
  for (const candidate of sources) {
    if (candidate === catalogPath) continue;
    const text = fs.readFileSync(candidate, "utf8");
    for (const site of registrationSites(text)) {
      for (const keys of bindingsIn(site)) registered.add(normalizeShortcut(keys));
    }
  }

  const documentedKeys = new Set([...documented].map(normalizeShortcut));
  const gaps = [];

  const undocumented = [...registered].filter((keys) => !documentedKeys.has(keys));
  if (undocumented.length > 0) {
    gaps.push(
      `${undocumented.length} registered shortcut(s) absent from the catalog in ` +
        `src/lib/shortcuts.ts: ${undocumented.slice(0, 8).join(", ")}`,
    );
  }

  // The other direction, which the one-way check could never see: a shortcut
  // documented in the help dialog and the tooltips that nothing binds any more.
  // To a user this is worse than an undocumented binding — the app advertises a
  // key that does nothing.
  const unregistered = [...documentedKeys].filter(
    (keys) => !registered.has(keys) && !(keys in SHORTCUTS_WITHOUT_LITERAL_REGISTRATION),
  );
  if (unregistered.length > 0) {
    gaps.push(
      `${unregistered.length} documented shortcut(s) with no registration found: ` +
        `${unregistered.slice(0, 8).join(", ")}`,
    );
  }

  // The exemptions must not outlive their reason, or they become a way to hide
  // a shortcut that really has stopped working.
  for (const keys of Object.keys(SHORTCUTS_WITHOUT_LITERAL_REGISTRATION)) {
    if (registered.has(keys)) {
      gaps.push(
        `'${keys}' is exempt from the registration check but is now registered ` +
          `literally; remove it from SHORTCUTS_WITHOUT_LITERAL_REGISTRATION.`,
      );
    } else if (!documentedKeys.has(keys)) {
      gaps.push(
        `'${keys}' is exempt from the registration check but is no longer in the ` +
          `catalog; remove it from SHORTCUTS_WITHOUT_LITERAL_REGISTRATION.`,
      );
    }
  }

  return gaps;
}

/**
 * Documented shortcuts that no literal registration can prove.
 *
 * Each entry is a claim that the shortcut works for a reason static analysis
 * cannot see. They are re-checked above, so an entry cannot quietly become a
 * cover for a genuinely dead binding.
 */
const SHORTCUTS_WITHOUT_LITERAL_REGISTRATION = {
  "MOD+B":
    "Bound from the SIDEBAR_KEYBOARD_SHORTCUT constant in " +
    "src/lib/components/ui/sidebar/context.svelte.ts, so no key literal appears at the call site.",
  "MOD+X":
    "Cut is handled by the webview's native editing keymap. The app documents it " +
    "in the Edit menu and the editor context menu without binding it itself.",
  "MOD+V":
    "Paste is handled by the webview's native editing keymap, for the same reason as Mod+X.",
};

/**
 * Normalize a key string so the app's several vocabularies are comparable.
 *
 * CodeMirror writes `Mod-Shift-z` where the catalog writes `Mod+Shift+Z`. The
 * lookbehind rewrites only a hyphen sitting between two word characters, so the
 * literal `-` and `=` in bindings like `Mod+-` and `Mod+=` survive intact.
 */
function normalizeShortcut(keys) {
  return keys.replace(/(?<=\w)-(?=\w)/g, "+").toUpperCase();
}

/**
 * The argument spans of every call that registers a key binding.
 *
 * Returns raw source text, bracket-balanced from the call's opening bracket, so
 * the extraction below cannot wander into unrelated object literals.
 */
function registrationSites(text) {
  const OPENERS = { "(": ")", "[": "]", "{": "}" };
  const spans = [];

  // The four shapes a registration takes. `: HotkeyBinding` covers the common
  // indirection where bindings are declared as a typed array and handed to
  // `registerHotkeys(theArray)` later — the keys are in the declaration, not at
  // the call, so scoping to call sites alone would miss every one of them.
  const REGISTRATION_FORMS =
    /registerHotkeys?\s*\(|use:hotkey\s*=|keymap\.of\s*\(|:\s*HotkeyBinding(?:\[\])?\s*=/g;

  for (const match of text.matchAll(REGISTRATION_FORMS)) {
    let index = match.index + match[0].length - 1;
    // `use:hotkey={...}` has its bracket after the `=`; the call forms are
    // already sitting on theirs.
    while (index < text.length && !(text[index] in OPENERS)) index += 1;
    if (index >= text.length) continue;

    const stack = [OPENERS[text[index]]];
    const start = index;
    index += 1;
    while (index < text.length && stack.length > 0) {
      const character = text[index];
      if (character in OPENERS) stack.push(OPENERS[character]);
      else if (character === stack[stack.length - 1]) stack.pop();
      index += 1;
    }
    spans.push(text.slice(start, index));
  }

  // The string-first `registerHotkey("Mod+S", callback)` overload: the key is
  // the call's first argument rather than a property, so it is captured here.
  for (const match of text.matchAll(/registerHotkeys?\s*\(\s*["'`]([^"'`]+)["'`]/g)) {
    spans.push(`{key:"${match[1]}"}`);
  }

  return spans;
}

/**
 * Every key binding written inside one registration span.
 *
 * Handles the three shapes a `RegisterableHotkey` takes: a plain string, an
 * array of strings, and the structured descriptor `{ key: "=", mod: true }` —
 * the last of which the previous regex silently reduced to `=`, losing every
 * modifier and reporting drift that did not exist.
 */
function bindingsIn(span) {
  const found = [];

  for (const match of span.matchAll(/\bkeys?\s*:\s*\{([^}]*)\}/g)) {
    const descriptor = match[1];
    const literal = /\bkey\s*:\s*["'`]([^"'`]+)["'`]/.exec(descriptor);
    if (!literal) continue;
    const flag = (name) => new RegExp(`\\b${name}\\s*:\\s*true\\b`).test(descriptor);
    const modifiers = [];
    if (flag("mod")) modifiers.push("Mod");
    if (flag("ctrl")) modifiers.push("Ctrl");
    if (flag("alt")) modifiers.push("Alt");
    if (flag("shift")) modifiers.push("Shift");
    found.push([...modifiers, literal[1]].join("+"));
  }

  // Strip the descriptor forms before the simpler patterns run, so a
  // descriptor's inner `key:` is not also collected without its modifiers.
  const remainder = span.replace(/\bkeys?\s*:\s*\{[^}]*\}/g, "");

  for (const match of remainder.matchAll(/\bkeys?\s*:\s*["'`]([^"'`]+)["'`]/g)) {
    found.push(match[1]);
  }
  for (const match of remainder.matchAll(/\bkeys?\s*:\s*\[([^\]]*)\]/g)) {
    for (const literal of match[1].matchAll(/["'`]([^"'`]+)["'`]/g)) found.push(literal[1]);
  }

  return found;
}

function tauriCommandSlice(name) {
  const source = fs.readFileSync(
    path.join(REPO_ROOT, "src-tauri", "src", "command_names.rs"),
    "utf8",
  );
  const slice = new RegExp(
    `pub const ${name}:\\s*&\\[&str\\]\\s*=\\s*&\\[([^\\]]*)\\]`,
  ).exec(source);
  if (!slice) return [];
  return [...slice[1].matchAll(/"([a-z0-9_]+)"/g)].map((match) => match[1]);
}

function tauriCommands() {
  return [
    ...tauriCommandSlice("APP_COMMANDS"),
    ...tauriCommandSlice("E2E_COMMANDS"),
  ];
}

/** Reconcile command inventory, handler registration, and both capabilities. */
function tauriSurfaceDrift() {
  const appCommands = tauriCommandSlice("APP_COMMANDS");
  const e2eCommands = tauriCommandSlice("E2E_COMMANDS");
  const handlerSource = fs.readFileSync(
    path.join(REPO_ROOT, "src-tauri", "src", "lib.rs"),
    "utf8",
  );
  const defaultCapability = JSON.parse(
    fs.readFileSync(
      path.join(REPO_ROOT, "src-tauri", "capabilities", "default.json"),
      "utf8",
    ),
  );
  const e2eCapability = JSON.parse(
    fs.readFileSync(
      path.join(REPO_ROOT, "src-tauri", "e2e-capabilities", "e2e.json"),
      "utf8",
    ),
  );
  const defaultPermissions = new Set(defaultCapability.permissions);
  const e2ePermissions = new Set(e2eCapability.permissions);
  const gaps = [];

  for (const command of [...appCommands, ...e2eCommands]) {
    if (!new RegExp(`::${command}\\s*,`).test(handlerSource)) {
      gaps.push(`IPC command '${command}' is inventoried but missing from generate_handler!.`);
    }
  }
  for (const command of appCommands) {
    const permission = `allow-${command.replaceAll("_", "-")}`;
    if (!defaultPermissions.has(permission)) {
      gaps.push(`Application command '${command}' is missing '${permission}' in default.json.`);
    }
  }
  for (const command of e2eCommands) {
    const permission = `allow-${command.replaceAll("_", "-")}`;
    if (!e2ePermissions.has(permission)) {
      gaps.push(`E2E command '${command}' is missing '${permission}' in e2e.json.`);
    }
  }
  return gaps;
}

// ── Reconcile ──────────────────────────────────────────────────────────────
const problems = [];
const note = (kind, detail) => problems.push({ kind, detail });

const requirements = loadRequirements();
const claims = collectClaims();
const byId = new Map(requirements.map((requirement) => [requirement.id, requirement]));

// 1. Registry hygiene.
const seenIds = new Set();
for (const requirement of requirements) {
  if (seenIds.has(requirement.id)) {
    note("duplicate-requirement", `'${requirement.id}' is listed more than once.`);
  }
  seenIds.add(requirement.id);

  if ((requirement.coverage === "rust" || requirement.coverage === "static") && !requirement.reason) {
    note("missing-reason", `'${requirement.id}' defers elsewhere without naming where.`);
  }
  if (requirement.coverage === "manual" && (!requirement.reason || !requirement.owner)) {
    note(
      "missing-reason",
      `'${requirement.id}' is a manual deferral without both a reason and an owner.`,
    );
  }
}

// 2. Every claim must resolve, be executable, and be unique.
const claimCounts = new Map();
for (const claim of claims) {
  if (!claim.id) {
    note("unresolvable-claim", `${claim.where}: scenario() called without a literal id.`);
    continue;
  }
  if (!byId.has(claim.id)) {
    note("unknown-claim", `${claim.where}: '${claim.id}' is not in the registry.`);
    continue;
  }
  if (!claim.executable) {
    note("inert-claim", `${claim.where}: '${claim.id}' is claimed without a test body.`);
    continue;
  }
  const requirement = byId.get(claim.id);
  if (requirement.coverage !== "e2e") {
    note(
      "misplaced-claim",
      `${claim.where}: '${claim.id}' is coverage '${requirement.coverage}' but claimed by a scenario.`,
    );
  }
  // A functional behavior claimed from `specs/visual/` becomes non-blocking the
  // moment the visual job is made advisory — the claim still counts, so the
  // coverage number stays flat while the gate quietly disappears.
  const requirementTier = tierOf(requirement);
  const specTier = tierOfSpec(claim.spec);
  if (requirementTier !== specTier) {
    note(
      "wrong-tier",
      `${claim.where}: '${claim.id}' is a ${requirementTier} behavior claimed from the ${specTier} tier.`,
    );
  }
  claimCounts.set(claim.id, (claimCounts.get(claim.id) ?? 0) + 1);
}

for (const [id, count] of claimCounts) {
  if (count > 1) {
    note("duplicate-claim", `'${id}' is claimed by ${count} scenarios; it must be exactly one.`);
  }
}

// 3. Every e2e requirement must be claimed.
const unclaimed = requirements
  .filter((requirement) => requirement.coverage === "e2e")
  .filter((requirement) => !claimCounts.has(requirement.id))
  .map((requirement) => requirement.id);

// 4. Source catalogs must be accounted for somewhere.
const catalogGaps = [];
const transformIds = transformationIds();
const rustTransformIds = rustTransformationIds();
const hasTransformFamilies = requirements.some(
  (requirement) => requirement.area === "transformations" && requirement.coverage === "rust",
);
if (transformIds.length > 0 && !hasTransformFamilies) {
  catalogGaps.push(
    `${transformIds.length} transformation ids exist but no requirement assigns registry correctness to a lower-level test.`,
  );
}
const frontendOnlyTransforms = transformIds.filter((id) => !rustTransformIds.includes(id));
const backendOnlyTransforms = rustTransformIds.filter((id) => !transformIds.includes(id));
if (frontendOnlyTransforms.length > 0 || backendOnlyTransforms.length > 0) {
  catalogGaps.push(
    `Transformation registries drifted: frontend-only [${frontendOnlyTransforms.join(", ")}], ` +
      `backend-only [${backendOnlyTransforms.join(", ")}].`,
  );
}

catalogGaps.push(...shortcutDrift());
catalogGaps.push(...tauriSurfaceDrift());

const commands = tauriCommands();

// Cancellation: reconciled in both directions against the exact evidence map.
// Forwards catches a new cancellable command nobody mapped; backwards catches a
// mapping that names a requirement the registry does not contain.
const cancellationCommands = commands.filter(
  (command) => command.startsWith("cancel_") || EXTRA_CANCELLATION_COMMANDS.includes(command),
);
for (const command of cancellationCommands) {
  const requirementId = CANCELLATION_EVIDENCE[command];
  if (!requirementId) {
    catalogGaps.push(
      `Cancellable command '${command}' has no entry in CANCELLATION_EVIDENCE; ` +
        `map it to the behavior that proves it.`,
    );
  } else if (!byId.has(requirementId)) {
    note(
      "missing-evidence",
      `Cancellation of '${command}' maps to '${requirementId}', which is not in the registry.`,
    );
  } else if (
    byId.get(requirementId).coverage === "e2e" &&
    !claimCounts.has(requirementId)
  ) {
    note(
      "missing-evidence",
      `Cancellation of '${command}' maps to unclaimed behavior '${requirementId}'.`,
    );
  }
}
for (const [command, requirementId] of Object.entries(CANCELLATION_EVIDENCE)) {
  if (!cancellationCommands.includes(command)) {
    note(
      "stale-evidence",
      `CANCELLATION_EVIDENCE maps '${command}' (-> '${requirementId}') but no such command exists.`,
    );
  }
}

// Settings: every persisted key must name the behavior that proves it survives
// the round trip. A new key added to `AppSettings` fails here until it does.
for (const key of settingKeys()) {
  const requirementId = SETTINGS_EVIDENCE[key];
  if (!requirementId) {
    catalogGaps.push(
      `Setting '${key}' has no entry in SETTINGS_EVIDENCE; name the behavior that proves it persists.`,
    );
  } else if (!byId.has(requirementId)) {
    note(
      "missing-evidence",
      `Setting '${key}' maps to '${requirementId}', which is not in the registry.`,
    );
  } else if (
    byId.get(requirementId).coverage === "e2e" &&
    !claimCounts.has(requirementId)
  ) {
    note(
      "missing-evidence",
      `Setting '${key}' maps to unclaimed behavior '${requirementId}'.`,
    );
  }
}

// ── Report ─────────────────────────────────────────────────────────────────
const e2eTotal = requirements.filter((requirement) => requirement.coverage === "e2e").length;
const claimed = claimCounts.size;
const rust = requirements.filter((requirement) => requirement.coverage === "rust").length;
const staticChecks = requirements.filter((r) => r.coverage === "static").length;
const manual = requirements.filter((requirement) => requirement.coverage === "manual").length;

console.log("e2e coverage audit");
console.log(`  requirements:   ${requirements.length} total`);
console.log(`  packaged e2e:   ${claimed}/${e2eTotal} claimed by an executable scenario`);
console.log(`  lower-level:    ${rust} assigned to Rust/unit tests`);
console.log(`  source audits:  ${staticChecks} proven by static analysis here`);
console.log(`  manual:         ${manual} deferred with a reason and an owner`);
console.log(
  `  catalogs:       ${transformIds.length} transformations, ${commands.length} IPC commands`,
);

if (unclaimed.length > 0) {
  console.log(`\n  ${unclaimed.length} behavior(s) not yet claimed by a scenario:`);
  for (const id of unclaimed) console.log(`    - ${id}`);
}
for (const gap of catalogGaps) console.log(`\n  catalog gap: ${gap}`);

// ── The ratchet ────────────────────────────────────────────────────────────
//
// Compared by identity, not by count. A count-based ratchet lets a newly added
// gap hide behind a newly closed one, which is exactly the regression the audit
// exists to catch. New gaps are fatal even outside strict mode; stale entries
// are fatal too, so the allowlist cannot rot into a permanent exemption list.
const newGaps = unclaimed.filter((id) => !KNOWN_GAPS.has(id));
for (const id of newGaps) {
  note(
    "new-gap",
    `'${id}' is unclaimed and is not in KNOWN_GAPS. Write the scenario, or ` +
      `record it deliberately.`,
  );
}
const closedGaps = [...KNOWN_GAPS].filter((id) => !unclaimed.includes(id));
for (const id of closedGaps) {
  note(
    "stale-gap",
    `'${id}' is listed in KNOWN_GAPS but is now claimed. Delete it from the list.`,
  );
}

if (problems.length > 0) {
  console.error(`\n${problems.length} coverage problem(s):`);
  for (const problem of problems) console.error(`  [${problem.kind}] ${problem.detail}`);
}

// Problems are always fatal: they mean the registry and the specs disagree.
// Unclaimed behaviors are fatal only in strict mode, so the audit can report
// the remaining backlog while the suite is still being built out.
const fatal = problems.length > 0 || (STRICT && (unclaimed.length > 0 || catalogGaps.length > 0));
if (fatal) {
  console.error(
    STRICT
      ? "\nCoverage audit failed (strict)."
      : "\nCoverage audit failed.",
  );
  process.exit(1);
}

if (unclaimed.length > 0) {
  console.log("\nAudit passed (non-strict): unclaimed behaviors are the remaining backlog.");
}
process.exit(0);
