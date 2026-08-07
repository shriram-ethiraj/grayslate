import { browser } from "@wdio/globals";

/**
 * The only sanctioned home for `browser.execute`.
 *
 * WebDriver cannot expose computed styles, loaded fonts, or CodeMirror's
 * rendered text, so a script bridge is unavoidable. What is avoidable — and
 * what this module exists to prevent — is scripts that *act*: the suite
 * previously used `browser.execute` to synthesize dblclick/pointerenter events,
 * set input values directly, assign `scrollTop`, focus elements instead of
 * clicking them, and inject nodes into the app's DOM. Those bypass the very
 * code paths the tests claim to cover.
 *
 * Rule: everything here reads. Nothing here mutates page state.
 * `e2e/scripts/lint-conventions.mjs` enforces that specs do not call
 * `browser.execute` directly.
 */

export interface EditorReadinessSnapshot {
  documentPath: string | null;
  documentLength: number | null;
  language: string | null;
  ready: boolean;
}

export interface PendingWorkSnapshot {
  phase: "booting" | "ready" | "closing";
  inFlight: number;
  commands: string[];
  tasks: string[];
  revision: number;
}

export interface PendingWorkFrames {
  first: PendingWorkSnapshot | null;
  second: PendingWorkSnapshot | null;
}

interface E2EWindowBridge {
  pending(): PendingWorkSnapshot;
}

/** Read the E2E-only application work tracker without starting any IPC. */
export async function readPendingWork(): Promise<PendingWorkSnapshot | null> {
  return browser.execute(() => {
    const bridge = (
      window as unknown as { __grayslateE2E?: E2EWindowBridge }
    ).__grayslateE2E;
    return bridge?.pending() ?? null;
  });
}

/**
 * Sample pending work on consecutive animation frames.
 *
 * A zero count at two ordinary polling instants can miss a short invoke that
 * starts and finishes between them. The monotonically increasing revision
 * makes that activity visible even when both endpoint counts are zero.
 */
export async function readPendingWorkAcrossFrames(): Promise<PendingWorkFrames> {
  return browser.executeAsync((done) => {
    const read = (): PendingWorkSnapshot | null => {
      const bridge = (
        window as unknown as { __grayslateE2E?: E2EWindowBridge }
      ).__grayslateE2E;
      return bridge?.pending() ?? null;
    };

    requestAnimationFrame(() => {
      const first = read();
      requestAnimationFrame(() => {
        done({ first, second: read() });
      });
    });
  });
}

/** Locale and timezone resolved by the packaged app's actual WebKit runtime. */
export async function readIntlEnvironment(): Promise<{
  locale: string;
  timeZone: string;
}> {
  return browser.execute(() => ({
    locale: new Intl.Collator(undefined, {
      numeric: true,
      sensitivity: "base",
    }).resolvedOptions().locale,
    timeZone: new Intl.DateTimeFormat().resolvedOptions().timeZone,
  }));
}

/** Whether the E2E-only motion override was installed before the UI mounted. */
export async function readHasDeterministicMotionStyle(): Promise<boolean> {
  return browser.execute(
    () => document.getElementById("grayslate-e2e-determinism") !== null,
  );
}

/** Read readiness from the same visible editor/status state a user sees. */
export async function readEditorReadiness(): Promise<EditorReadinessSnapshot> {
  return browser.execute(() => {
    const editor = document.querySelector<HTMLElement>("[data-testid='editor']");
    const content = editor?.querySelector<HTMLElement>(".cm-content");
    const loader = document.querySelector<HTMLElement>("[data-testid='editor-loader']");
    const title = document.querySelector<HTMLElement>("[data-testid='title-file-name']");
    const status = document.querySelector<HTMLElement>("[data-testid='status-length']");
    const language = document.querySelector<HTMLElement>("[data-testid='language-mode']");
    const rawLength = status?.dataset.docLength;
    const documentLength = rawLength === undefined ? null : Number(rawLength);
    const configuredLanguage = language?.dataset.languageMode;
    const effectiveLanguage = configuredLanguage === "auto"
      ? language?.dataset.detectedLanguage
      : configuredLanguage;
    const editorVisible =
      content !== undefined && content !== null && content.getClientRects().length > 0;

    return {
      documentPath: title?.dataset.documentPath ?? null,
      documentLength: Number.isFinite(documentLength) ? documentLength : null,
      language: effectiveLanguage ?? null,
      ready: editor !== null && editorVisible && loader === null && documentLength !== null,
    };
  });
}

/**
 * Read CodeMirror's rendered document text, preserving line breaks.
 *
 * This reads the DOM, so for a virtualized document it returns only what is
 * rendered. Assert on `data-doc-length` (see `readDocumentLength`) whenever the
 * document may exceed the viewport.
 */
export async function readEditorText(): Promise<string> {
  return browser.execute(() => {
    const content = document.querySelector<HTMLElement>(
      "[data-testid='editor'] .cm-content",
    );
    if (!content) throw new Error("CodeMirror content element is missing.");

    return content.innerText.replace(/\n$/, "");
  });
}

/**
 * The authoritative document length from the status bar.
 *
 * Survives CodeMirror viewport virtualization, unlike `readEditorText`.
 */
export async function readDocumentLength(): Promise<number | null> {
  return browser.execute(() => {
    const status = document.querySelector<HTMLElement>("[data-testid='status-length']");
    const raw = status?.dataset.docLength;
    if (raw === undefined) return null;
    const parsed = Number(raw);
    return Number.isFinite(parsed) ? parsed : null;
  });
}

/** Whether the live CodeMirror content element currently owns keyboard focus. */
export async function readEditorHasFocus(): Promise<boolean> {
  return browser.execute(() => {
    const content = document.querySelector("[data-testid='editor'] .cm-content");
    return content !== null && document.activeElement === content;
  });
}

/** Text currently selected in the webview document. */
export async function readDocumentSelectionText(): Promise<string> {
  return browser.execute(() => window.getSelection()?.toString() ?? "");
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

/** Current sidebar pane size as exposed by Paneforge's separator ARIA state. */
export async function readSidebarPaneSize(): Promise<number | null> {
  return browser.execute(() => {
    const handle = document.querySelector<HTMLElement>(
      "[data-testid='sidebar-resize-handle']",
    );
    const size = Number.parseFloat(handle?.getAttribute("aria-valuenow") ?? "");
    return Number.isFinite(size) ? size : null;
  });
}

/** Resolved computed-style values for one element, by `data-testid`. */
export async function readComputedStyle(
  testId: string,
  properties: string[],
): Promise<Record<string, string> | null> {
  return browser.execute((id, props) => {
    const element = document.querySelector<HTMLElement>(`[data-testid='${id}']`);
    if (!element) return null;
    const style = window.getComputedStyle(element);
    const result: Record<string, string> = {};
    for (const property of props) {
      result[property] = style.getPropertyValue(property);
    }
    return result;
  }, testId, properties);
}

/** Resolved computed-style values for an arbitrary CSS selector. */
export async function readComputedStyleBySelector(
  selector: string,
  properties: string[],
): Promise<Record<string, string> | null> {
  return browser.execute((sel, props) => {
    const element = document.querySelector<HTMLElement>(sel);
    if (!element) return null;
    const style = window.getComputedStyle(element);
    const result: Record<string, string> = {};
    for (const property of props) {
      result[property] = style.getPropertyValue(property);
    }
    return result;
  }, selector, properties);
}

/** Whether the document root currently carries the dark theme class. */
export async function readIsDarkTheme(): Promise<boolean> {
  return browser.execute(() => document.documentElement.classList.contains("dark"));
}

/** Read one `localStorage` key, for persistence assertions. */
export async function readLocalStorage(storageKey: string): Promise<string | null> {
  return browser.execute((k) => window.localStorage.getItem(k), storageKey);
}

/**
 * The text content of one CSV grid cell.
 *
 * `getText()` returns only *rendered* text, and the CSV virtualizer positions
 * rows absolutely, so WebDriver reports an empty string for cells that are
 * plainly visible on screen. `textContent` reads what the cell actually holds.
 * Cells are single-line, so nothing is lost by not using `innerText`.
 */
export async function readCellText(row: number, col: number): Promise<string | null> {
  return browser.execute((r, c) => {
    const cell = document.querySelector(`[data-row='${r}'][data-col='${c}']`);
    if (!cell) return null;
    const input = cell.querySelector("input");
    if (input) return (input as HTMLInputElement).value;
    return (cell.textContent ?? "").trim();
  }, row, col);
}

/** The text of every currently visible tooltip. */
export async function readVisibleTooltips(): Promise<string[]> {
  return browser.execute(() =>
    Array.from(document.querySelectorAll<HTMLElement>("[role='tooltip']"))
      .filter((tooltip) => tooltip.getClientRects().length > 0)
      .map((tooltip) => tooltip.textContent?.trim() ?? "")
      .filter(Boolean),
  );
}

/** Count elements matching a raw selector, for virtualization bounds checks. */
export async function countElements(selector: string): Promise<number> {
  return browser.execute((sel) => document.querySelectorAll(sel).length, selector);
}

/**
 * Read a global the app should never have set.
 *
 * Used by the Markdown sanitizer spec, which asserts the payload's *side
 * effect* never fired rather than merely that the markup was stripped.
 */
export async function readWindowGlobal(name: string): Promise<unknown> {
  return browser.execute(
    (key) => (window as unknown as Record<string, unknown>)[key] ?? null,
    name,
  );
}
