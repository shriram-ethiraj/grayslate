import { $, browser } from "@wdio/globals";
import { TIMEOUTS, INTERVALS } from "../config/timeouts.js";
import { MOD, SHIFT, pressMod, typeText } from "../driver/keys.js";
import {
  readDocumentLength,
  readEditorHasFocus,
  readEditorReadiness,
  readEditorText,
  type EditorReadinessSnapshot,
} from "../driver/probe.js";
import { waitFor, waitForIdle } from "../driver/wait.js";
import { clickSelector } from "../driver/interact.js";
import { byTestId, clickTestId, existsTestId } from "./common.js";

/** The CodeMirror editing surface. */

export function content(): ReturnType<typeof $> {
  return $("[data-testid='editor'] .cm-content");
}

/**
 * Put the keyboard focus in the editor, and prove it landed.
 *
 * Clicking `.cm-content` directly is unreliable: CodeMirror gives it a large
 * `padding-bottom` (hundreds of pixels) so the document can scroll past the
 * last line, which puts the element's *centre* — where WebDriver clicks —
 * outside the viewport for a short document. The driver reports the click as
 * successful and no `mousedown` is ever dispatched, so focus silently stays
 * where it was. Every later keystroke then goes somewhere else and the failure
 * surfaces as an unrelated assertion about document text.
 *
 * Clicking the first rendered line instead targets a small box that is always
 * on screen, and the focus check below turns any remaining failure into a
 * message about focus rather than about text.
 */
export async function focus(): Promise<void> {
  const element = await content();
  await element.waitForDisplayed({
    timeout: TIMEOUTS.editor,
    timeoutMsg: "The CodeMirror content element never became visible.",
  });

  const firstLine = await $("[data-testid='editor'] .cm-line");
  const selector = (await firstLine.isExisting())
    ? "[data-testid='editor'] .cm-line"
    : "[data-testid='editor'] .cm-content";

  await waitFor(
    async () => {
      if (await readEditorHasFocus()) return true;

      // Route through the shared click helper so a lingering tooltip or menu
      // overlay is dismissed first; a raw `.click()` here just reports
      // "element click intercepted" and retries into the same overlay forever.
      await clickSelector(selector).catch(() => {
        // A transient interception is retried by the next poll.
      });
      if (await readEditorHasFocus()) return true;

      // On Linux/X11 without a window manager, a native clipboard reader can
      // leave the webview without an active DOM element. After the next file
      // open, WebDriver's primary-button click moves CodeMirror's caret but
      // does not always restore contenteditable focus. Exercise the editor's
      // real context-menu path as a bounded fallback: its production
      // pointerdown handler focuses the view, and Escape closes the menu.
      await firstLine.click({ button: "right" }).catch(() => {
        // A transient interception is retried by the next poll.
      });
      if (await existsTestId("editor-context-menu")) {
        await browser.keys("Escape");
      }
      return readEditorHasFocus();
    },
    {
      message: "Clicking the editor did not give it keyboard focus.",
      timeoutMs: TIMEOUTS.ui,
      intervalMs: INTERVALS.fast,
    },
  );
}

export interface ReadyOptions {
  documentPath?: string;
  documentLength?: number;
  language?: string;
  timeoutMs?: number;
}

/**
 * Wait for the E2E bootstrap to mount the editor shell, or surface its explicit
 * error immediately instead of allowing every scenario in the worker to time out.
 */
export async function waitForBootstrap(): Promise<void> {
  let bootstrapError: string | null = null;
  await waitFor(
    async () => {
      const errorElement = await $("[data-testid='e2e-bootstrap-error']");
      if (await errorElement.isExisting()) {
        bootstrapError = await errorElement.getText();
        return true;
      }
      return (await $("[data-testid='editor']")).isExisting();
    },
    {
      message: "The E2E runtime never mounted the editor shell or an explicit bootstrap error.",
      timeoutMs: TIMEOUTS.editor,
      intervalMs: INTERVALS.fast,
    },
  );

  if (bootstrapError !== null) {
    throw new Error(bootstrapError);
  }
}

/**
 * Wait for a specific editor/document state.
 *
 * First reaches the requested visible state, then proves tracked application
 * work has stopped across two rendered frames and validates the state again.
 */
export async function waitUntilReady(
  options: ReadyOptions = {},
): Promise<EditorReadinessSnapshot> {
  let latest: EditorReadinessSnapshot | undefined;
  const timeoutMs = options.timeoutMs ?? TIMEOUTS.editor;
  const deadline = Date.now() + timeoutMs;

  const matches = (snapshot: EditorReadinessSnapshot): boolean =>
    snapshot.ready &&
    (options.documentPath === undefined ||
      snapshot.documentPath === options.documentPath) &&
    (options.documentLength === undefined ||
      snapshot.documentLength === options.documentLength) &&
    (options.language === undefined || snapshot.language === options.language);

  while (Date.now() < deadline) {
    const remaining = Math.max(1, deadline - Date.now());
    await waitFor(
      async () => {
        latest = await readEditorReadiness();
        return matches(latest);
      },
      {
        message:
          `Editor never reached the requested state ${JSON.stringify(options)}. ` +
          `Last snapshot: ${JSON.stringify(latest ?? null)}`,
        timeoutMs: remaining,
        intervalMs: INTERVALS.fast,
      },
    );

    await waitForIdle({
      timeoutMs: Math.max(1, deadline - Date.now()),
      message: "The editor reached its target state but application work did not settle.",
    });
    latest = await readEditorReadiness();
    if (matches(latest)) return latest;
  }

  if (!latest) throw new Error("Editor readiness completed without a snapshot.");
  throw new Error(
    `Editor changed after becoming idle. Last snapshot: ${JSON.stringify(latest)}`,
  );
}

/** Wait until the live document text satisfies a predicate. */
export async function waitForText(
  predicate: (text: string) => boolean,
  message: string,
  timeoutMs: number = TIMEOUTS.ui,
): Promise<void> {
  let observed = "";
  await waitFor(
    async () => {
      observed = await readEditorText();
      return predicate(observed);
    },
    {
      message: () => `${message} Last observed: ${JSON.stringify(observed.slice(0, 200))}`,
      timeoutMs,
      intervalMs: INTERVALS.slow,
    },
  );
}

/** Wait for the exact document text. */
export async function waitForExactText(expected: string, timeoutMs = TIMEOUTS.ui): Promise<void> {
  await waitForText(
    (text) => text === expected,
    `Editor text never became ${JSON.stringify(expected.slice(0, 120))}.`,
    timeoutMs,
  );
}

/**
 * Wait for the authoritative document length.
 *
 * Use this instead of a text comparison for large documents: `readEditorText`
 * only sees what CodeMirror has rendered, so a virtualized document reports far
 * less text than it holds.
 */
export async function waitForLength(expected: number, timeoutMs = TIMEOUTS.heavy): Promise<void> {
  await waitFor(async () => (await readDocumentLength()) === expected, {
    message: `Document length never became ${expected}.`,
    timeoutMs,
    intervalMs: INTERVALS.slow,
  });
}

/** Replace the whole document through the keyboard path a user would use. */
export async function replaceText(text: string): Promise<void> {
  await focus();
  await pressMod("a");
  await typeText(text);
  await waitForExactText(text);
}

/** Type at the current cursor without clearing the document. */
export async function type(text: string): Promise<void> {
  await typeText(text);
}

/** Select `count` characters to the right of the cursor. */
export async function selectRight(count: number): Promise<void> {
  for (let i = 0; i < count; i += 1) {
    await browser.keys([SHIFT, "ArrowRight"]);
  }
}

export async function selectAll(): Promise<void> {
  await focus();
  await pressMod("a");
}

export async function undo(): Promise<void> {
  await pressMod("z");
}

export async function redo(): Promise<void> {
  // macOS binds redo to Mod+Shift+Z; Windows and Linux use Mod+Y.
  if (process.platform === "darwin") await browser.keys([MOD, SHIFT, "z"]);
  else await pressMod("y");
}

export async function save(): Promise<void> {
  await pressMod("s");
}

/** Open the editor's own context menu at the current caret. */
export async function openContextMenu(): Promise<void> {
  const element = await content();
  await element.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: "The editor was not visible when opening its context menu.",
  });
  await element.click({ button: "right" });
  const menu = await byTestId("editor-context-menu");
  await menu.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: "The editor context menu never opened.",
  });
}

/**
 * Right-click a specific token on a rendered line.
 *
 * The JSON-aware items exist only when the click resolves to a JSON syntax
 * node: the extension reads `posAtCoords` at the pointer, not the caret, so
 * moving the caret first proves nothing. `.cm-line` is a block element spanning
 * the editor's full width, so its centre is almost always past the end of the
 * text and resolves to the line end. Offsetting from the left edge is what puts
 * the pointer inside the first token.
 *
 * `columnOffsetPx` is measured from the line's left edge.
 */
export async function openContextMenuOnLine(
  lineIndex: number,
  columnOffsetPx: number,
): Promise<void> {
  const line = await $(`[data-testid='editor'] .cm-line:nth-of-type(${lineIndex + 1})`);
  await line.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: `Line ${lineIndex} was not rendered when opening the context menu.`,
  });

  const { width } = await line.getSize();
  await line.click({
    button: "right",
    // WDIO offsets are relative to the element's centre.
    x: Math.round(columnOffsetPx - width / 2),
    y: 0,
  });

  const menu = await byTestId("editor-context-menu");
  await menu.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: "The editor context menu never opened.",
  });
}

/** Whether a context-menu item is present for the current click target. */
export async function hasContextMenuItem(item: string): Promise<boolean> {
  return existsTestId(`editor-context-${item}`);
}

/** Choose one item from the editor context menu. */
export async function chooseContextMenuItem(
  item:
    | "copy-path"
    | "copy-key"
    | "copy-value"
    | "cut"
    | "copy"
    | "select-all"
    | "word-wrap",
): Promise<void> {
  await clickTestId(`editor-context-${item}`);
}

export { readEditorText as text, readDocumentLength as length };
