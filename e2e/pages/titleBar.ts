import { browser } from "@wdio/globals";
import { TIMEOUTS } from "../config/timeouts.js";
import { waitFor } from "../driver/wait.js";
import {
  attributeOf,
  byTestId,
  clickTestId,
  existsTestId,
  isAriaDisabled,
  pressEscape,
  waitForTestId,
} from "./common.js";

/**
 * The title bar: the app menus, the document name, the dirty indicator, and the
 * native window controls.
 *
 * On macOS the menus are native (`src-tauri/src/menu/mod.rs`) and unreachable
 * from WebDriver; the in-window menubar here is the Windows/Linux surface.
 */

// ── Menus ──────────────────────────────────────────────────────────────────

type FileItem = "new-slate" | "open-file" | "save" | "save-as" | "settings";
type EditItem =
  | "undo"
  | "redo"
  | "cut"
  | "copy"
  | "go-to-line"
  | "find"
  | "find-files"
  | "replace"
  | "word-wrap"
  | "select-all";
type ViewItem = "increase-font" | "decrease-font" | "reset-font";
type HelpItem = "keyboard-shortcuts" | "check-updates" | "about";

async function openMenu(menu: "file" | "edit" | "view" | "help"): Promise<void> {
  const trigger = menu === "help" ? "app-help-menu" : `menu-${menu}`;
  const sentinel = {
    file: "menu-new-slate",
    edit: "menu-undo",
    view: "menu-increase-font",
    help: "menu-about",
  }[menu];

  for (let attempt = 1; attempt <= 3; attempt += 1) {
    await clickTestId(trigger);
    const item = await byTestId(sentinel);
    try {
      await item.waitForDisplayed({
        timeout: 1_500,
        timeoutMsg: `The ${menu} menu did not open.`,
      });
      return;
    } catch (error) {
      if (attempt === 3) throw error;
      await pressEscape();
    }
  }
}

export async function fileMenu(item: FileItem): Promise<void> {
  await openMenu("file");
  await clickTestId(`menu-${item}`);
}

export async function editMenu(item: EditItem): Promise<void> {
  await openMenu("edit");
  await clickTestId(`menu-${item}`);
}

export async function viewMenu(item: ViewItem): Promise<void> {
  await openMenu("view");
  await clickTestId(`menu-${item}`);
}

export async function helpMenu(item: HelpItem): Promise<void> {
  await openMenu("help");
  await clickTestId(item === "keyboard-shortcuts" ? "help-keyboard-shortcuts" : `menu-${item}`);
}

/** Read a menu item's state without activating it. Leaves the menu open. */
export async function readMenuItemAttribute(
  menu: "file" | "edit" | "view" | "help",
  testId: string,
  attribute: string,
): Promise<string | null> {
  await openMenu(menu);
  return attributeOf(testId, attribute);
}

// ── Document identity and dirty state ──────────────────────────────────────

export async function documentPath(): Promise<string | null> {
  return attributeOf("title-file-name", "data-document-path");
}

/**
 * Whether the current local file has unsaved changes.
 *
 * Reads the title-bar asterisk, which renders exactly when
 * `editorState.isDirty && currentFileSource === "local"`. The Save button is
 * not usable as a signal: `TooltipButton` disables via `aria-disabled` only, so
 * WebDriver's `isEnabled()` always returns true.
 */
export async function isDirty(): Promise<boolean> {
  return existsTestId("title-dirty-indicator");
}

/**
 * Wait for the dirty flag to settle.
 *
 * A poller rather than a point read: a save clears the flag only after its IPC
 * round-trip resolves, which can lag the disk write a file assertion observes.
 */
export async function waitForDirty(expected: boolean): Promise<void> {
  await waitFor(async () => (await isDirty()) === expected, {
    message: `Document dirty state never became ${expected}.`,
    timeoutMs: TIMEOUTS.ui,
    intervalMs: 150,
  });
}

/** Wait for the toolbar Save control's enabled state. */
export async function waitForSaveEnabled(expected: boolean): Promise<void> {
  await waitForTestId("action-save");
  await waitFor(async () => (await isAriaDisabled("action-save")) !== expected, {
    message: `Save action enabled state never became ${expected}.`,
    timeoutMs: TIMEOUTS.ui,
    intervalMs: 150,
  });
}

// ── Native window controls ─────────────────────────────────────────────────

export async function minimizeWindow(): Promise<void> {
  const control = await byTestId("window-minimize");
  await control.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: "The window minimize control never became visible.",
  });

  // WebKitWebDriver's high-level element click waits for post-click window
  // interactability. That can never settle after the click itself suspends the
  // webview. A W3C pointer action still performs a real move/down/up on the
  // production control, but returns as soon as the events are dispatched.
  await browser
    .action("pointer")
    .move({ origin: control })
    .down()
    .up()
    .perform();
}

export async function toggleMaximizeWindow(): Promise<void> {
  await clickTestId("window-maximize");
}

export async function closeWindow(): Promise<void> {
  await clickTestId("window-close");
}

/** The maximize control's own view of the window state. */
export async function isMaximized(): Promise<boolean> {
  return (await attributeOf("window-maximize", "data-maximized")) === "true";
}

export async function waitForMaximized(expected: boolean): Promise<void> {
  await waitFor(async () => (await isMaximized()) === expected, {
    message: `Window maximized state never became ${expected}.`,
    timeoutMs: TIMEOUTS.ui,
  });
}
