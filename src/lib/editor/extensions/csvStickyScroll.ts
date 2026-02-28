/**
 * csvStickyScroll.ts
 *
 * CodeMirror 6 extension that freezes the CSV header row at the top of the
 * editor — like a spreadsheet's frozen first row.
 *
 * Built on top of the generic `stickyScroll` engine.  This module provides
 * **only** the scope definition (line 1 → last line).  All rendering is
 * handled by the generic engine via DOM cloning — the sticky header
 * automatically mirrors whatever decorations the editor applied to line 1
 * (rainbow colours, custom themes, etc.) with zero coupling to those
 * extensions.
 *
 * ## Usage
 *
 * ```ts
 * import { csvStickyScroll } from "./csvStickyScroll";
 *
 * new EditorView({
 *     extensions: [csvStickyScroll],
 * });
 * ```
 *
 * Pair with `csvRainbowHighlight` for full rainbow column colouring in
 * both the editor body and the sticky header:
 *
 * ```ts
 * import { csvRainbowHighlight } from "./csvRainbowHighlight";
 * import { csvStickyScroll } from "./csvStickyScroll";
 *
 * new EditorView({
 *     extensions: [csvRainbowHighlight, csvStickyScroll],
 * });
 * ```
 */

import type { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { createStickyScroll } from "./stickyScroll";
import type { StickyScope } from "./stickyScroll";

// ---------------------------------------------------------------------------
// Scope parsing
// ---------------------------------------------------------------------------

/**
 * CSV "scopes" — a single scope from line 1 (header) to the last line.
 *
 * The generic sticky-scroll engine shows the `openLine` whenever it has
 * scrolled above the viewport top and the `closeLine` is still below it.
 * Since the close line is the very end of the document, the header stays
 * pinned for the entire scroll range — exactly the "frozen header row"
 * behaviour we want.
 */
function parseCsvScopes(view: EditorView): StickyScope[] {
    const lineCount = view.state.doc.lines;
    // Need at least 2 lines for the sticky header to be meaningful.
    if (lineCount < 2) return [];

    return [
        {
            openLine: 1,
            closeLine: lineCount,
        },
    ];
}

// ---------------------------------------------------------------------------
// Extension
// ---------------------------------------------------------------------------

/**
 * CSV sticky-scroll extension.
 *
 * Provides scope detection only — rendering is fully handled by the
 * generic sticky-scroll engine's DOM cloning pipeline, which
 * automatically captures all decorations applied to the header line.
 */
export const csvStickyScroll: Extension = createStickyScroll({
    class: "cm-csv-sticky-scroll",
    computeScopes: parseCsvScopes,
    maxLines: 1,
});
