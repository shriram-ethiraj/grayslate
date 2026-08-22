export type FocusedTextEditAction =
  | "undo"
  | "redo"
  | "cut"
  | "copy"
  | "selectAll";

const NON_TEXT_INPUT_TYPES = new Set([
  "button",
  "checkbox",
  "color",
  "file",
  "hidden",
  "image",
  "radio",
  "range",
  "reset",
  "submit",
]);

function getFocusedTextControl(
  editorDom: HTMLElement | undefined,
): HTMLInputElement | HTMLTextAreaElement | HTMLElement | undefined {
  const activeElement = document.activeElement;
  if (!(activeElement instanceof HTMLElement)) return undefined;

  // CodeMirror's content surface is contenteditable, but its state and history
  // must remain owned by CodeMirror rather than the browser editing commands.
  if (editorDom?.contains(activeElement)) return undefined;

  if (activeElement instanceof HTMLInputElement) {
    return NON_TEXT_INPUT_TYPES.has(activeElement.type.toLowerCase())
      ? undefined
      : activeElement;
  }

  if (activeElement instanceof HTMLTextAreaElement) return activeElement;
  if (activeElement.isContentEditable) return activeElement;
  return undefined;
}

function selectTextControlContents(
  control: HTMLInputElement | HTMLTextAreaElement | HTMLElement,
): void {
  if (
    control instanceof HTMLInputElement ||
    control instanceof HTMLTextAreaElement
  ) {
    try {
      control.select();
    } catch {
      // Some input types expose text editing but reject programmatic
      // selection. The action is still claimed so it never reaches CodeMirror.
    }
    return;
  }

  const selection = window.getSelection();
  if (!selection) return;

  const range = document.createRange();
  range.selectNodeContents(control);
  selection.removeAllRanges();
  selection.addRange(range);
}

/**
 * Route an intercepted edit action to the focused browser text control.
 *
 * Custom macOS menu accelerators consume the original key event before
 * WKWebView can apply its normal input behavior, while WebKitGTK does not
 * reliably apply native undo/redo shortcuts to controlled inputs. Re-running
 * the browser edit command here preserves the focused control's own selection
 * and undo history on both paths.
 * Returning true means the action was claimed even if there was nothing to
 * copy, cut, undo, or redo, so the command must never fall through to the
 * document editor.
 */
export function handleFocusedTextEdit(
  action: FocusedTextEditAction,
  editorDom: HTMLElement | undefined,
): boolean {
  const control = getFocusedTextControl(editorDom);
  if (!control) return false;

  if (action === "selectAll") {
    selectTextControlContents(control);
    return true;
  }

  try {
    document.execCommand(action);
  } catch {
    // Clipboard commands may be rejected by a webview policy. The focused
    // control still owns the action; falling through would edit the document.
  }
  return true;
}
