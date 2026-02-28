/**
 * csvRainbowHighlight.ts
 *
 * Rainbow CSV column colouring for CodeMirror 6 — inspired by the
 * "Rainbow CSV" VS Code extension.
 *
 * Each CSV column is assigned a cycling CSS class (`csv-col-0` … `csv-col-9`).
 * Two colour palettes are embedded via `EditorView.baseTheme()`:
 *   - Default (light) palette: deep, saturated hues that stay legible on
 *     white/light backgrounds.
 *   - Dark (`&dark`) palette: brighter, pastel-leaning hues optimised for
 *     dark editor backgrounds.
 *
 * IMPORTANT: This extension is used *instead of* a Lezer grammar for CSV
 * files (e.g. `codemirror-lang-csv`).  If a grammar is active, its
 * `HighlightStyle` tags will override our decoration colours once the async
 * parse completes.  Keep the language-extensions entry for CSV pointing
 * exclusively at this module.
 *
 * Performance notes:
 *   - Only visible-viewport lines are decorated (via `view.visibleRanges`).
 *   - Delimiter detection samples only the first 5 000 chars and caches.
 *   - Viewport-only scrolls reuse the cached decoration set unless the
 *     visible line range actually changed.
 */

import { ViewPlugin, Decoration, EditorView } from "@codemirror/view";
import type { DecorationSet, ViewUpdate } from "@codemirror/view";
import { RangeSetBuilder } from "@codemirror/state";
import type { Extension } from "@codemirror/state";
import {
    createStickyHeaderPanel,
    stickyHeaderBaseTheme,
} from "./stickyHeader";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const NUM_COLORS = 10;

/**
 * Pre-built mark decorations — one per colour slot.
 * Using named CSS classes (rather than inline styles) lets the
 * `baseTheme()` palettes swap cleanly between light/dark without needing to
 * redispatch decorations on theme change.
 */
const FIELD_MARKS = Array.from({ length: NUM_COLORS }, (_, i) =>
    Decoration.mark({ class: `csv-col-${i}` }),
);

// ---------------------------------------------------------------------------
// Delimiter detection
// ---------------------------------------------------------------------------

const CANDIDATE_DELIMITERS = [",", "\t", ";", "|"] as const;

/**
 * Heuristically determine the field delimiter by counting occurrences of each
 * candidate in the first `sampleSize` characters of the document.
 * Returns the delimiter with the highest occurrence count; falls back to `,`.
 */
function detectDelimiter(sample: string): string {
    let best = ",";
    let bestCount = -1;
    for (const d of CANDIDATE_DELIMITERS) {
        let count = 0;
        let from = 0;
        while ((from = sample.indexOf(d, from)) !== -1) {
            count++;
            from += d.length;
        }
        if (count > bestCount) {
            bestCount = count;
            best = d;
        }
    }
    return best;
}

// ---------------------------------------------------------------------------
// RFC-4180 field-range extractor
// ---------------------------------------------------------------------------

interface FieldRange {
    /** Inclusive start within the line string. */
    from: number;
    /** Exclusive end within the line string (character *after* last field char). */
    to: number;
}

/**
 * Parse a single CSV line and return the character ranges of each field.
 *
 * Handles:
 *  - Unquoted fields separated by `delimiter`
 *  - Double-quoted fields (RFC 4180), including embedded delimiters and
 *    doubled-quote escape sequences `""`
 *
 * @param line      Raw line text (no trailing newline)
 * @param delimiter Field separator string (typically one char)
 */
function getFieldRanges(line: string, delimiter: string): FieldRange[] {
    const ranges: FieldRange[] = [];
    const dlen = delimiter.length;
    let i = 0;

    while (i <= line.length) {
        const fieldStart = i;

        if (i < line.length && line[i] === '"') {
            // ── Quoted field ──────────────────────────────────────────────
            let j = i + 1;
            while (j < line.length) {
                if (line[j] === '"') {
                    if (line[j + 1] === '"') {
                        // doubled-quote escape: skip both
                        j += 2;
                    } else {
                        // closing quote
                        j++;
                        break;
                    }
                } else {
                    j++;
                }
            }
            // j is now one past the closing quote (or at line.length if
            // the field was never properly closed — be tolerant)
            ranges.push({ from: fieldStart, to: j });

            if (j < line.length && line.slice(j, j + dlen) === delimiter) {
                i = j + dlen;
            } else {
                // end of line (or malformed — stop gracefully)
                break;
            }
        } else {
            // ── Unquoted field ────────────────────────────────────────────
            const delimIdx = line.indexOf(delimiter, i);
            if (delimIdx === -1) {
                ranges.push({ from: fieldStart, to: line.length });
                break;
            } else {
                ranges.push({ from: fieldStart, to: delimIdx });
                i = delimIdx + dlen;
            }
        }
    }

    return ranges;
}

// ---------------------------------------------------------------------------
// Sticky header — CSV-specific render + change detection
// ---------------------------------------------------------------------------

/**
 * Populate `dom` with rainbow-coloured column spans for line 1.
 * Direct DOM manipulation — no framework overhead.
 */
function renderCsvHeader(dom: HTMLElement, view: EditorView): void {
    dom.replaceChildren();
    const lineText = view.state.doc.line(1).text;
    if (!lineText) return;

    const delimiter = detectDelimiter(view.state.doc.sliceString(0, 5000));
    const fields = getFieldRanges(lineText, delimiter);

    for (let col = 0; col < fields.length; col++) {
        const { from, to } = fields[col];

        if (col > 0) {
            const sep = document.createElement("span");
            sep.className = "csv-hdr-sep";
            sep.textContent = delimiter;
            dom.appendChild(sep);
        }

        const span = document.createElement("span");
        span.className = `csv-col-${col % NUM_COLORS}`;
        span.textContent = lineText.slice(from, to);
        dom.appendChild(span);
    }
}

/** Track line-1 text so we can skip unnecessary re-renders. */
let _lastCsvHeaderText = "";

const csvStickyHeader: Extension = createStickyHeaderPanel({
    class: "csv-sticky-header",
    anchorLine: 1,
    getLineNumber: () => 1,
    render(dom, view) {
        _lastCsvHeaderText = view.state.doc.line(1).text;
        renderCsvHeader(dom, view);
    },
    shouldRerender(update) {
        if (!update.docChanged) return false;
        const newText = update.state.doc.line(1).text;
        if (newText === _lastCsvHeaderText) return false;
        return true;
    },
});

// ---------------------------------------------------------------------------
// ViewPlugin
// ---------------------------------------------------------------------------

/**
 * Build a `DecorationSet` for the currently visible lines.
 * Iterates only over `view.visibleRanges` for efficiency.
 */
function buildDecorations(view: EditorView, delimiter: string): DecorationSet {
    const builder = new RangeSetBuilder<Decoration>();
    const { doc } = view.state;

    for (const { from, to } of view.visibleRanges) {
        const firstLine = doc.lineAt(from).number;
        const lastLine = doc.lineAt(to).number;

        for (let ln = firstLine; ln <= lastLine; ln++) {
            const line = doc.line(ln);
            // Skip blank lines early to avoid allocating a FieldRange[]
            if (!line.text) continue;

            const fields = getFieldRanges(line.text, delimiter);

            for (let col = 0; col < fields.length; col++) {
                const { from: fFrom, to: fTo } = fields[col];
                const absFrom = line.from + fFrom;
                const absTo = line.from + fTo;

                // Skip zero-width or inverted ranges to keep the builder happy
                if (absFrom >= absTo) continue;

                builder.add(absFrom, absTo, FIELD_MARKS[col % NUM_COLORS]);
            }
        }
    }

    return builder.finish();
}

/**
 * Hash the visible-range boundaries so we can cheaply detect whether the
 * viewport actually moved to different lines (as opposed to a horizontal
 * scroll or an unrelated update).  This lets us skip full decoration
 * rebuilds on scroll events that don't change which lines are visible.
 */
function viewportKey(view: EditorView): string {
    return view.visibleRanges
        .map((r) => `${r.from}-${r.to}`)
        .join(",");
}

class CsvRainbowPlugin {
    decorations: DecorationSet;

    /** Cached delimiter — only re-detected when the document changes. */
    private delimiter: string;

    /** Fingerprint of the last decorated viewport to avoid redundant rebuilds. */
    private lastViewportKey: string;

    /** Cached sample used for delimiter detection (avoids redundant work). */
    private lastSample: string = "";

    constructor(view: EditorView) {
        this.delimiter = this.refreshDelimiter(view);
        this.lastViewportKey = viewportKey(view);
        this.decorations = buildDecorations(view, this.delimiter);
    }

    update(update: ViewUpdate) {
        if (update.docChanged) {
            this.delimiter = this.refreshDelimiter(update.view);
            this.lastViewportKey = viewportKey(update.view);
            this.decorations = buildDecorations(update.view, this.delimiter);
            return;
        }

        if (update.viewportChanged) {
            const key = viewportKey(update.view);
            // Only rebuild when the visible line range actually shifted.
            if (key !== this.lastViewportKey) {
                this.lastViewportKey = key;
                this.decorations = buildDecorations(update.view, this.delimiter);
            }
        }
    }

    private refreshDelimiter(view: EditorView): string {
        // Sample only the first 5 000 chars — enough for reliable detection
        // without paying the cost of `doc.toString()` on large files.
        const sample = view.state.doc.sliceString(0, 5000);
        if (sample !== this.lastSample) {
            this.lastSample = sample;
            return detectDelimiter(sample);
        }
        return this.delimiter;
    }
}

// ---------------------------------------------------------------------------
// Colour palettes via baseTheme
// ---------------------------------------------------------------------------

/**
 * Theme extension that registers the 10 `csv-col-N` colours.
 *
 * Default (light) rules use deep, saturated tones.
 * `&dark` variants use brighter pastels suited to dark backgrounds.
 *
 * The `&dark` prefix is a CodeMirror 6 `baseTheme` convention: it scopes
 * the rule to when the editor root carries the `.cm-editor` dark-theme class.
 */
// We use `!important` to guarantee rainbow colours always win, even if
// residual syntax-highlight styles or theme base rules target the same
// token spans.  Without it a `HighlightStyle` for `tags.string` would
// silently override the decorations.
const rainbowTheme: Extension = EditorView.baseTheme({
    // Dimmed delimiter glyphs between header fields
    ".csv-hdr-sep": {
        opacity: "0.4",
    },
    // ── Light palette ────────────────────────────────────────────────────────
    ".csv-col-0": { color: "#7a5200 !important" },  // dark amber
    ".csv-col-1": { color: "#0b6b7a !important" },  // dark teal
    ".csv-col-2": { color: "#1e6b40 !important" },  // dark green
    ".csv-col-3": { color: "#1a4080 !important" },  // dark blue
    ".csv-col-4": { color: "#9b2400 !important" },  // brick red
    ".csv-col-5": { color: "#6a2e8c !important" },  // dark purple
    ".csv-col-6": { color: "#7a4c00 !important" },  // dark orange
    ".csv-col-7": { color: "#9e2070 !important" },  // deep pink
    ".csv-col-8": { color: "#106655 !important" },  // dark sea-green
    ".csv-col-9": { color: "#8a1f48 !important" },  // dark rose

    // ── Dark palette ─────────────────────────────────────────────────────────
    "&dark .csv-col-0": { color: "#e6b840 !important" },  // golden yellow
    "&dark .csv-col-1": { color: "#3ecece !important" },  // cyan-teal
    "&dark .csv-col-2": { color: "#80d860 !important" },  // lime green
    "&dark .csv-col-3": { color: "#7ca8e8 !important" },  // sky blue
    "&dark .csv-col-4": { color: "#f08060 !important" },  // coral
    "&dark .csv-col-5": { color: "#c48ae8 !important" },  // lavender
    "&dark .csv-col-6": { color: "#f0a840 !important" },  // orange
    "&dark .csv-col-7": { color: "#e070c0 !important" },  // hot pink
    "&dark .csv-col-8": { color: "#48daa0 !important" },  // sea green
    "&dark .csv-col-9": { color: "#f07898 !important" },  // rose
});

// ---------------------------------------------------------------------------
// Public export
// ---------------------------------------------------------------------------

export const csvRainbowHighlight: Extension[] = [
    ViewPlugin.fromClass(CsvRainbowPlugin, {
        decorations: (plugin) => plugin.decorations,
    }),
    csvStickyHeader,
    stickyHeaderBaseTheme,
    rainbowTheme,
];
