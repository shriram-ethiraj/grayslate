import { TIMEOUTS } from "../config/timeouts.js";
import { invokeInApp } from "../driver/invoke.js";
import { waitFor } from "../driver/wait.js";
import {
  attributeOf,
  clickTestId,
  isDisplayedTestId,
  pressEscape,
  textOf,
  waitForTestId,
} from "./common.js";
import { fileMenu } from "./titleBar.js";

/**
 * The Settings dialog.
 *
 * Every setting is persisted through Rust (`storage.rs`, SQLite), so a change
 * made here survives a restart — which is the property the persistence specs
 * exist to prove.
 */

export type StartupBehavior = "new" | "last";
export type IndentMode = "spaces" | "tab";
export type LineEnding = "lf" | "crlf";
export type CharacterEncoding =
  | "utf-8"
  | "utf-8-bom"
  | "utf-16le"
  | "utf-16be"
  | "windows-1252";

export async function open(): Promise<void> {
  await fileMenu("settings");
  await waitForTestId("settings-dialog");
}

export async function close(): Promise<void> {
  await pressEscape();
  await waitForTestId("settings-dialog", { reverse: true });
}

export async function showPane(pane: "general" | "editor"): Promise<void> {
  await clickTestId(`settings-pane-${pane}`);
}

/**
 * Choose an option from one of the dialog's selects.
 *
 * Both the trigger and each option carry a `data-testid`, so this never has to
 * match on a visible label — labels change with copy edits, and the previous
 * helpers built XPath expressions by interpolating the label text.
 */
/**
 * Choose an option and wait for the value to be *committed*.
 *
 * The listbox closing is not the signal: it unmounts on click, before the
 * change has been written through `set_app_setting` and reflected back. Waiting
 * on the trigger's own label means the assertion that follows is reading
 * settled state, not an in-flight one.
 */
async function choose(setting: string, value: string, expectedLabel: string): Promise<void> {
  const option = `settings-${setting}-${value}`;
  await clickTestId(`settings-${setting}`);
  await waitForTestId(option);
  await clickTestId(option);

  await waitFor(async () => !(await isDisplayedTestId(option)), {
    message: `The '${setting}' option list never closed after choosing '${value}'.`,
    timeoutMs: TIMEOUTS.ui,
  });

  let label = "";
  await waitFor(
    async () => {
      label = (await textOf(`settings-${setting}`)).trim();
      return label.toLowerCase().includes(expectedLabel.toLowerCase());
    },
    {
      message:
        `Setting '${setting}' never committed to '${value}'. ` +
        `Trigger still reads ${JSON.stringify(label)}.`,
      timeoutMs: TIMEOUTS.ui,
    },
  );
}

/**
 * A distinctive fragment of each option's visible label.
 *
 * Mirrors the option tables in `src/lib/components/SettingsDialog.svelte` and
 * `CHARACTER_ENCODING_OPTIONS` in `src/lib/state/appSettings.svelte.ts`. Only a
 * fragment is needed — enough to tell the options apart, little enough that a
 * copy edit does not break the suite.
 */
const LABEL_FRAGMENTS = {
  startup: { new: "new slate", last: "Reopen last" },
  "indent-mode": { spaces: "Spaces", tab: "Tab" },
  "line-ending": { lf: "LF", crlf: "CRLF" },
  "character-encoding": {
    "utf-8": "UTF-8",
    "utf-8-bom": "UTF-8 with BOM",
    "utf-16le": "UTF-16 LE",
    "utf-16be": "UTF-16 BE",
    "windows-1252": "Windows-1252",
  },
} as const;

export async function setStartupBehavior(value: StartupBehavior): Promise<void> {
  await showPane("general");
  await choose("startup", value, LABEL_FRAGMENTS.startup[value]);
}

export async function setIndentMode(value: IndentMode): Promise<void> {
  await showPane("editor");
  await choose("indent-mode", value, LABEL_FRAGMENTS["indent-mode"][value]);
}

export async function setIndentSize(value: number): Promise<void> {
  await showPane("editor");
  await choose("indent-size", String(value), String(value));
}

export async function setLineEnding(value: LineEnding): Promise<void> {
  await showPane("editor");
  await choose("line-ending", value, LABEL_FRAGMENTS["line-ending"][value]);
}

export async function setCharacterEncoding(value: CharacterEncoding): Promise<void> {
  await showPane("editor");
  await choose("character-encoding", value, LABEL_FRAGMENTS["character-encoding"][value]);
}

export async function setConfirmBeforeDelete(enabled: boolean): Promise<void> {
  await showPane("general");
  const current = (await attributeOf("settings-confirm-delete", "aria-checked")) === "true";
  if (current === enabled) return;
  await clickTestId("settings-confirm-delete");
  await waitFor(
    async () =>
      ((await attributeOf("settings-confirm-delete", "aria-checked")) === "true") === enabled,
    {
      message: `'Confirm before deleting' never became ${enabled}.`,
      timeoutMs: TIMEOUTS.ui,
    },
  );
}

export async function automaticUpdateChecksEnabled(): Promise<boolean> {
  await showPane("general");
  return (await attributeOf("settings-automatic-update-checks", "aria-checked")) === "true";
}

export async function setAutomaticUpdateChecks(enabled: boolean): Promise<void> {
  await showPane("general");
  if ((await automaticUpdateChecksEnabled()) === enabled) return;
  await clickTestId("settings-automatic-update-checks");
  await waitFor(
    async () => (await automaticUpdateChecksEnabled()) === enabled,
    {
      message: `'Automatically check for updates' never became ${enabled}.`,
      timeoutMs: TIMEOUTS.ui,
    },
  );
}

/** The visible label on a setting's trigger, for round-trip assertions. */
export async function triggerLabel(setting: string): Promise<string> {
  return textOf(`settings-${setting}`);
}

/**
 * Wait for the Rust-owned settings store, not just the trigger label.
 *
 * Setting setters intentionally update the UI immediately and persist over
 * IPC. Restart tests must not race that write, so they settle on the same
 * `get_all_settings` boundary startup reads.
 */
export async function waitForPersistedValue(
  key: string,
  expected: string,
): Promise<void> {
  let observed: string | undefined;
  await waitFor(
    async () => {
      const values = await invokeInApp<Record<string, string>>("get_all_settings");
      observed = values[key];
      return observed === expected;
    },
    {
      message: () =>
        `Setting '${key}' never persisted as ${JSON.stringify(expected)}. ` +
        `Last observed: ${JSON.stringify(observed)}`,
      timeoutMs: TIMEOUTS.disk,
    },
  );
}

/** Read one value from the Rust-owned settings store. */
export async function persistedValue(key: string): Promise<string | undefined> {
  const values = await invokeInApp<Record<string, string>>("get_all_settings");
  return values[key];
}
