import fs from "node:fs";
import path from "node:path";
import { $, browser } from "@wdio/globals";
import { homeDirectory } from "./sandbox.js";

/** Source fixtures committed under `e2e/fixtures/`. */
export const fixturesDir = path.resolve(process.cwd(), "e2e", "fixtures");
/** Sandbox location for "external" (non-slate) files, outside the notes root. */
export const externalRoot = path.join(homeDirectory, "external");

// WebDriver key codes.
const CONTROL = "\uE009";
const META = "\uE03D";
const ENTER = "\uE007";
const SHIFT = "\uE008";
const ALT = "\uE00A";
const HOME = "\uE011";
const ARROW_RIGHT = "\uE014";
const ARROW_DOWN = "\uE015";
const DELETE = "\uE017";
/** The platform modifier the app binds "Mod+" shortcuts to. */
export const MOD = process.platform === "darwin" ? META : CONTROL;
export { ALT, ARROW_DOWN, ARROW_RIGHT, DELETE, ENTER, HOME, SHIFT };

// ---------------------------------------------------------------------------
// Tauri IPC from the webview
//
// WebDriver runs scripts in the webview context, where `__TAURI_INTERNALS__`
// is available. This is the same access path the security IPC spec uses.
// ---------------------------------------------------------------------------

interface TauriInternals {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
}

interface InvokeResult<T> {
  value?: T;
  error?: string;
}

/** The camelCase descriptor returned by the open/save grant commands. */
export interface DocumentDescriptor {
  documentId: string;
  generation: number;
  displayPath: string;
  fileName: string;
  source: string;
  writable: boolean;
}

async function rawInvoke<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<InvokeResult<T>> {
  try {
    return await browser.executeAsync((name, payload, done) => {
      const internals = (window as unknown as { __TAURI_INTERNALS__: TauriInternals })
        .__TAURI_INTERNALS__;
      internals
        .invoke<T>(name, payload)
        .then((value) => done({ value }))
        .catch((error: unknown) => done({ error: String(error) }));
    }, command, args);
  } catch (error) {
    return { error: String(error) };
  }
}

/** Invoke a Tauri command in the webview, throwing on backend error. */
export async function invokeInApp<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  const result = await rawInvoke<T>(command, args);
  if (result.error !== undefined) {
    throw new Error(`invoke ${command} failed: ${result.error}`);
  }
  return result.value as T;
}

// ---------------------------------------------------------------------------
// Editor + keyboard
// ---------------------------------------------------------------------------

/** The CodeMirror content element. */
export function editorContent(): ReturnType<typeof $> {
  return $("[data-testid='editor'] .cm-content");
}

export async function focusEditor(): Promise<void> {
  const el = await editorContent();
  await el.waitForDisplayed();
  await el.click();
}

/**
 * Type text one character at a time with a full down/up per key. WebKit treats
 * adjacent identical key-downs as auto-repeat, so `browser.keys(string)` can
 * drop repeated characters; this preserves the exact bytes.
 */
export async function typeText(text: string): Promise<void> {
  // Keep action payloads small. WebKitWebDriver can reorder events in a very
  // long single action sequence under load, which corrupts otherwise valid
  // CodeMirror input while still reporting success.
  const batchSize = 24;
  for (let start = 0; start < text.length; start += batchSize) {
    const action = browser.action("key");
    for (const character of text.slice(start, start + batchSize)) {
      const key = character === "\n" ? ENTER : character;
      action.down(key).pause(25).up(key).pause(25);
    }
    await action.perform();
  }
}

/** Replace the full CodeMirror document through the same keyboard path a user uses. */
export async function replaceEditorText(text: string): Promise<void> {
  await focusEditor();
  await pressMod("a");
  await typeText(text);
  await waitForEditorText((value) => value === text);
}

/** Read CodeMirror's rendered document text, preserving line breaks. */
export async function readEditorText(): Promise<string> {
  return browser.execute(() => {
    const content = document.querySelector<HTMLElement>("[data-testid='editor'] .cm-content");
    if (!content) throw new Error("CodeMirror content element is missing.");
    return content.innerText.replace(/\n$/, "");
  });
}

/** Wait until the live CodeMirror document satisfies a predicate. */
export async function waitForEditorText(
  predicate: (text: string) => boolean,
  timeoutMs = 10_000,
): Promise<void> {
  await browser.waitUntil(async () => predicate(await readEditorText()), {
    timeout: timeoutMs,
    interval: 200,
    timeoutMsg: "The editor content did not reach the expected state.",
  });
}

/** Press the platform modifier plus a key, e.g. `pressMod("s")` for Save. */
export async function pressMod(key: string): Promise<void> {
  await browser.keys([MOD, key]);
}

/** Click any element carrying the given `data-testid`. */
export async function clickTestId(testId: string): Promise<void> {
  const el = await $(`[data-testid='${testId}']`);
  await el.waitForClickable();
  await el.click();
}

/** The status-bar language button (carries detected + active mode attributes). */
export function languageMode(): ReturnType<typeof $> {
  return $("[data-testid='language-mode']");
}

/** Wait until content detection settles on `lang` (the `Auto (...)` value). */
export async function waitForDetectedLanguage(lang: string, timeoutMs = 10_000): Promise<void> {
  await (await languageMode()).waitForDisplayed();
  await browser.waitUntil(async () =>
    (await languageMode()).getAttribute("data-detected-language").then((value) => value === lang), {
    timeout: timeoutMs,
    interval: 250,
    timeoutMsg: `Detected language never became '${lang}'.`,
  });
}

/** Wait until the effective (saved-file) language mode becomes `lang`. */
export async function waitForLanguageMode(lang: string, timeoutMs = 10_000): Promise<void> {
  await browser.waitUntil(async () =>
    (await languageMode()).getAttribute("data-language-mode").then((value) => value === lang), {
    timeout: timeoutMs,
    interval: 250,
    timeoutMsg: `Language mode never became '${lang}'.`,
  });
}

// ---------------------------------------------------------------------------
// File lifecycle
// ---------------------------------------------------------------------------

/** Copy a committed fixture into the sandbox's external (non-slate) directory. */
export function provisionExternalFixture(name: string): string {
  fs.mkdirSync(externalRoot, { recursive: true });
  const dest = path.join(externalRoot, name);
  fs.copyFileSync(path.join(fixturesDir, name), dest);
  return dest;
}

/** Write a generated fixture outside the notes root. */
export function provisionExternalText(name: string, content: string): string {
  fs.mkdirSync(externalRoot, { recursive: true });
  const dest = path.join(externalRoot, name);
  fs.writeFileSync(dest, content, "utf8");
  return dest;
}

/**
 * Open a fixture as an external local file through the real authorized-open
 * path (`e2e_open_path` grants + emits the production open event). Returns the
 * sandbox path the file was provisioned at.
 */
export async function openExternalFixture(name: string): Promise<string> {
  const dest = provisionExternalFixture(name);
  await invokeInApp<DocumentDescriptor | null>("e2e_open_path", { path: dest });
  return dest;
}

/** Provision and open generated text through the real authorized-open path. */
export async function openExternalText(name: string, content: string): Promise<string> {
  const dest = provisionExternalText(name, content);
  await invokeInApp<DocumentDescriptor | null>("e2e_open_path", { path: dest });
  return dest;
}

/** Grant a Save-As target path through the real authorization path. */
export async function grantSavePath(targetPath: string): Promise<DocumentDescriptor | null> {
  return invokeInApp<DocumentDescriptor | null>("e2e_save_path", { path: targetPath });
}

/** Create a fresh untitled slate via the File menu. */
export async function newSlate(): Promise<void> {
  await clickTestId("menu-file");
  await clickTestId("menu-new-slate");
}

/** Wait until a file on disk satisfies `predicate` (defaults to "exists"). */
export async function waitForFile(
  filePath: string,
  predicate: (content: string) => boolean = () => true,
  timeoutMs = 15_000,
): Promise<void> {
  await browser.waitUntil(
    () => {
      try {
        return predicate(fs.readFileSync(filePath, "utf8"));
      } catch {
        return false;
      }
    },
    {
      timeout: timeoutMs,
      interval: 200,
      timeoutMsg: `File condition never met for ${filePath}`,
    },
  );
}

/** Synthesize a large CSV in the sandbox (for the >100k-row safety case). */
export function writeLargeCsv(filePath: string, rows: number): void {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  const stream = fs.openSync(filePath, "w");
  fs.writeSync(stream, "id,name,value\n");
  for (let i = 1; i <= rows; i += 1) {
    fs.writeSync(stream, `${i},row-${i},${i * 2}\n`);
  }
  fs.closeSync(stream);
}

// ---------------------------------------------------------------------------
// Sidebar
// ---------------------------------------------------------------------------

/** Ensure the library sidebar is expanded (toggles with Mod+B if collapsed). */
export async function ensureSidebarOpen(): Promise<void> {
  const tab = await $("[data-testid='sidebar-tab-unified']");
  const open = await tab.isClickable().catch(() => false);
  if (!open) {
    await clickTestId("sidebar-toggle");
    await tab.waitForClickable();
  }
}

/** The sidebar card element for a given file path. */
export function sidebarCard(filePath: string): ReturnType<typeof $> {
  return $(`[data-card-path="${filePath}"]`);
}

export async function setFilterTab(tab: "unified" | "slates" | "local"): Promise<void> {
  await clickTestId(`sidebar-tab-${tab}`);
}

/** Open a file by clicking its sidebar card's open button. */
export async function openSidebarCard(filePath: string): Promise<void> {
  const card = await sidebarCard(filePath);
  await card.waitForDisplayed();
  const openButton = await card.$("button");
  await openButton.click();
}

/** Visible sidebar paths in their current rendered order. */
export async function readSidebarPaths(): Promise<string[]> {
  return browser.execute(() =>
    Array.from(document.querySelectorAll<HTMLElement>("[data-card-path]"))
      .filter((card) => card.offsetParent !== null)
      .map((card) => card.dataset.cardPath ?? "")
      .filter(Boolean),
  );
}

// ---------------------------------------------------------------------------
// Transformations
// ---------------------------------------------------------------------------

/** Open the transformations palette and run one action by id. */
export async function runTransform(actionId: string, focus = true): Promise<void> {
  if (focus) await focusEditor();
  await pressMod("k");
  const palette = await $("[data-testid='transformations-palette']");
  await palette.waitForDisplayed();
  const item = await $(`[data-testid='transform-item-${actionId}']`);
  await item.waitForDisplayed();
  await item.click();
}

// ---------------------------------------------------------------------------
// CSV table
// ---------------------------------------------------------------------------

export async function enterCsvTable(): Promise<void> {
  await clickTestId("action-table-view");
  const table = await $("[data-testid='csv-table']");
  await table.waitForDisplayed();
}

export async function exitCsvTable(): Promise<void> {
  await clickTestId("action-plain-csv");
}

/** A CSV grid cell at (row, col); col -1 is the row-number gutter. */
export function csvCell(row: number, col: number): ReturnType<typeof $> {
  return $(`[data-row='${row}'][data-col='${col}']`);
}
