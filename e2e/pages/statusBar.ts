import { TIMEOUTS } from "../config/timeouts.js";
import { waitFor, waitForAttribute } from "../driver/wait.js";
import {
  attributeOf,
  clickTestId,
  isDisplayedTestId,
  textOf,
  waitForTestId,
  pressEscape,
} from "./common.js";

/**
 * The status bar: document length, cursor position, indentation, line ending,
 * character encoding, and language mode.
 *
 * In CSV table mode the bar swaps to `status-csv-info` and hides the
 * length/indent/EOL/encoding items, which is itself a behavior worth asserting.
 */

export type LineEnding = "lf" | "crlf";
export type CharacterEncoding =
  | "utf-8"
  | "utf-8-bom"
  | "utf-16le"
  | "utf-16be"
  | "windows-1252";

// ── Line endings ───────────────────────────────────────────────────────────

export async function eol(): Promise<string | null> {
  return attributeOf("status-eol", "data-eol");
}

export async function waitForEol(expected: LineEnding): Promise<void> {
  await waitForAttribute("status-eol", "data-eol", expected, {
    message: `Status bar line ending never became '${expected}'.`,
  });
}

/** Pick a line ending from the status-bar picker, through the real dialog. */
export async function selectEol(expected: LineEnding): Promise<void> {
  await clickTestId("status-eol");
  await waitForTestId("eol-picker-dialog");
  await clickTestId("eol-select-trigger");
  await clickTestId(`eol-item-${expected}`);
  await waitForEol(expected);
  await pressEscape();
  await waitForTestId("eol-picker-dialog", { reverse: true });
}

// ── Character encoding ─────────────────────────────────────────────────────

export async function encoding(): Promise<string | null> {
  return attributeOf("status-encoding", "data-encoding");
}

export async function waitForEncoding(expected: CharacterEncoding): Promise<void> {
  await waitForAttribute("status-encoding", "data-encoding", expected, {
    message: `Status bar encoding never became '${expected}'.`,
    timeoutMs: TIMEOUTS.editor,
  });
}

async function runEncodingAction(
  target: CharacterEncoding,
  action: "save" | "reopen",
): Promise<void> {
  await clickTestId("status-encoding");
  await waitForTestId("encoding-picker-dialog");
  await clickTestId("encoding-select-trigger");
  await clickTestId(`encoding-item-${target}`);
  await clickTestId(action === "save" ? "encoding-save" : "encoding-reopen");
  await waitForTestId("encoding-picker-dialog", { reverse: true });
}

/** Convert the active document to `target` and write it. */
export async function saveWithEncoding(target: CharacterEncoding): Promise<void> {
  await runEncodingAction(target, "save");
  await waitForEncoding(target);
}

/**
 * Reinterpret the active document's original bytes as `target`.
 *
 * Deliberately does not handle the unsaved-changes prompt a local file raises:
 * the caller owns that choice so specs can assert the prompt's timing and
 * behavior. Managed slates complete silently.
 */
export async function reopenWithEncoding(target: CharacterEncoding): Promise<void> {
  await runEncodingAction(target, "reopen");
}

// ── Indentation ────────────────────────────────────────────────────────────

export async function indentLabel(): Promise<string> {
  return textOf("status-indent");
}

/**
 * Open the indentation picker, or leave it open if it already is.
 *
 * Idempotent on purpose: `status-indent` is a toggle, so a second call would
 * close the dialog a caller believed it had just opened.
 */
export async function openIndentPicker(): Promise<void> {
  if (await isDisplayedTestId("indent-picker")) return;
  await clickTestId("status-indent");
  await waitForTestId("indent-picker");
}

/** Choose the indentation mode, including the one-shot "Detect from content". */
export async function selectIndentMode(
  mode: "default" | "spaces" | "tab" | "detect",
): Promise<void> {
  await openIndentPicker();
  await clickTestId("indent-mode-trigger");
  await clickTestId(`indent-mode-${mode}`);
}

/** Choose the indentation width. Only meaningful for spaces or tabs. */
export async function selectIndentSize(size: number): Promise<void> {
  await clickTestId("indent-size-trigger");
  await clickTestId(`indent-size-${size}`);
}

export async function closeIndentPicker(): Promise<void> {
  await pressEscape();
  await waitForTestId("indent-picker", { reverse: true });
}

/** Wait for the status bar's indentation summary to match. */
export async function waitForIndentLabel(fragment: string): Promise<void> {
  let observed = "";
  await waitFor(
    async () => {
      observed = await indentLabel();
      return observed.includes(fragment);
    },
    {
      message: () => `Status bar indentation never showed '${fragment}'. Last: ${JSON.stringify(observed)}`,
      timeoutMs: TIMEOUTS.ui,
    },
  );
}

// ── Document metrics ───────────────────────────────────────────────────────

export async function documentLength(): Promise<string | null> {
  return attributeOf("status-length", "data-doc-length");
}

export async function lineCount(): Promise<string | null> {
  return attributeOf("status-length", "data-line-count");
}

export async function cursorLabel(): Promise<string> {
  return textOf("status-goto-line");
}

export async function csvInfo(): Promise<string> {
  return textOf("status-csv-info");
}

// ── Language ───────────────────────────────────────────────────────────────

export async function detectedLanguage(): Promise<string | null> {
  return attributeOf("language-mode", "data-detected-language");
}

export async function languageMode(): Promise<string | null> {
  return attributeOf("language-mode", "data-language-mode");
}

export async function waitForDetectedLanguage(expected: string): Promise<void> {
  await waitForAttribute("language-mode", "data-detected-language", expected, {
    message: `Detected language never became '${expected}'.`,
    timeoutMs: TIMEOUTS.editor,
  });
}

export async function waitForLanguageMode(expected: string): Promise<void> {
  await waitForAttribute("language-mode", "data-language-mode", expected, {
    message: `Language mode never became '${expected}'.`,
    timeoutMs: TIMEOUTS.editor,
  });
}

/** Override the editor language, or return it to automatic detection. */
export async function selectLanguage(language: string | "auto"): Promise<void> {
  await clickTestId("language-mode");
  await waitForTestId("language-picker-dialog");
  await clickTestId(language === "auto" ? "language-item-auto" : `language-item-${language}`);
  await waitForTestId("language-picker-dialog", { reverse: true });
  await waitForLanguageMode(language);
}
