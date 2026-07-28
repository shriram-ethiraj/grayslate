import { TIMEOUTS } from "../config/timeouts.js";
import { readIsDarkTheme, readLocalStorage } from "../driver/probe.js";
import { waitFor } from "../driver/wait.js";
import { clickTestId, isAriaDisabled } from "./common.js";
import { waitUntilReady } from "./editor.js";
import { fileMenu } from "./titleBar.js";

/** App-shell level actions that do not belong to any one surface. */

/**
 * Request a fresh slate without waiting for it.
 *
 * Use this when the request is expected to raise the unsaved-changes guard, so
 * the spec can drive the dialog. Use `newSlate()` when it should complete.
 */
export async function requestNewSlate(): Promise<void> {
  await fileMenu("new-slate");
}

/** Create a fresh untitled slate and wait for its session to mount. */
export async function newSlate(): Promise<void> {
  await requestNewSlate();
  await waitUntilReady({
    documentPath: "New Slate",
    documentLength: 0,
    timeoutMs: TIMEOUTS.editor,
  });
}

/** Whether the header's "New slate" button is unavailable (already blank). */
export async function isNewSlateDisabled(): Promise<boolean> {
  return isAriaDisabled("header-new-slate");
}

export async function toggleTheme(): Promise<void> {
  await clickTestId("theme-toggle");
}

export async function isDarkTheme(): Promise<boolean> {
  return readIsDarkTheme();
}

export async function waitForTheme(dark: boolean): Promise<void> {
  await waitFor(async () => (await isDarkTheme()) === dark, {
    message: `The app never switched to the ${dark ? "dark" : "light"} theme.`,
    timeoutMs: TIMEOUTS.ui,
  });
}

/**
 * Restore the theme to what it was.
 *
 * Specs must leave global state as they found it. The suite previously left the
 * theme inverted and the default indentation changed, and got away with it only
 * because the sandbox happened to be wiped between spec files.
 */
export async function restoreTheme(wasDark: boolean): Promise<void> {
  if ((await isDarkTheme()) === wasDark) return;
  await toggleTheme();
  await waitForTheme(wasDark);
}

/** Read a persisted UI preference, for cross-restart assertions. */
export async function storedPreference(key: string): Promise<string | null> {
  return readLocalStorage(key);
}
