/**
 * csvCellHeaderTooltip.ts
 *
 * CodeMirror 6 hover-tooltip extension that shows "Col N: <header>" when the
 * user hovers any CSV cell value in text mode — matching the VS Code /
 * Rainbow CSV column-identification experience.
 *
 * ## Performance model
 *
 * This extension is safe for documents of arbitrary size:
 *
 *  - `csvMetaField` (StateField) is computed once at document creation and
 *    re-computed only when `docChanged` is true.  Each recomputation reads at
 *    most 5 000 characters for delimiter detection and accesses only the first
 *    non-empty line for header parsing — O(log n + line_length), where n is
 *    total document length (CodeMirror Text is a balanced rope).
 *
 *  - `hoverTooltip` fires on user hover only; it reads the StateField in O(1)
 *    and calls `getFieldRanges` on the single hovered line — O(line_length),
 *    never a full-document scan.
 *
 * ## Tooltip label format
 *
 *   "Col 3: Email"   — header cell is non-blank
 *   "Col 3"          — header cell is blank or column index is out-of-range
 *
 * Clicking the tooltip copies the column header name (or "Col N" fallback)
 * to the clipboard.
 *
 * ## DOM structure (mirrors jsonKeyPath.ts)
 *
 *   <div class="cm-csv-header-tooltip">
 *     <span class="cm-csv-col-prefix">Col 3: </span>
 *     <span class="cm-csv-col-name">Email</span>   ← omitted when no header
 *   </div>
 *
 * Styles are defined in src/routes/layout.css alongside the JSON key-path
 * tooltip styles for consistent theming.
 */

import { StateField } from "@codemirror/state";
import type { EditorState, Extension } from "@codemirror/state";
import { hoverTooltip } from "@codemirror/view";
import { detectDelimiter, decodeFieldValue, getFieldRanges } from "./csvUtils";
import { toast } from "$lib/components/ui/sonner";

// ---------------------------------------------------------------------------
// CsvMeta — cached per-document header information
// ---------------------------------------------------------------------------

interface CsvMeta {
    /** Detected field separator (`,`, `\t`, `;`, `|`). */
    delimiter: string;
    /**
     * Decoded header labels from the first non-empty line of the document.
     * May contain empty strings for blank/missing header cells.
     */
    headers: string[];
    /** 1-based document line number of the detected header row. */
    headerLineNumber: number;
}

/**
 * Parse CSV metadata from the current EditorState.
 *
 * Reads at most 5 000 characters for delimiter detection and accesses only
 * the first non-empty document line for header extraction — safe for very
 * large files where `doc.toString()` would be prohibitively expensive.
 */
function computeCsvMeta(state: EditorState): CsvMeta | null {
    if (state.doc.length === 0) return null;

    // Delimiter detection uses the same 5 000-char budget as csvRainbowHighlight
    // so there is no additional cost on top of what's already paid per keystroke.
    const sample = state.doc.sliceString(0, 5000);
    if (!sample.trim()) return null;

    const delimiter = detectDelimiter(sample);

    // Walk document lines from the top until the first non-empty record.
    // For well-formed CSV files this is always line 1 (one iteration).
    const lineCount = state.doc.lines;
    for (let ln = 1; ln <= lineCount; ln++) {
        const line = state.doc.line(ln);
        if (!line.text.trim()) continue;

        const ranges = getFieldRanges(line.text, delimiter);
        const headers = ranges.map(({ from, to }, idx) => {
            const decoded = decodeFieldValue(line.text.slice(from, to));
            // Strip the UTF-8 BOM (\uFEFF) that some Windows editors (Excel,
            // Notepad) prepend to the very first character of a text file.
            return idx === 0 && decoded.charCodeAt(0) === 0xfeff
                ? decoded.slice(1)
                : decoded;
        });

        return { delimiter, headers, headerLineNumber: ln };
    }

    return null;
}

// ---------------------------------------------------------------------------
// StateField — header cache, invalidated only on docChanged
// ---------------------------------------------------------------------------

/**
 * StateField that holds the parsed CSV header metadata.
 *
 * It participates in every transaction but only re-parses when `docChanged`
 * is true, keeping hover latency near-zero for large files during normal
 * cursor movement, scrolling, or selection changes.
 */
const csvMetaField = StateField.define<CsvMeta | null>({
    create: computeCsvMeta,
    update(value, tr) {
        return tr.docChanged ? computeCsvMeta(tr.state) : value;
    },
});

// ---------------------------------------------------------------------------
// Tooltip hover handler
// ---------------------------------------------------------------------------

/**
 * Self-contained CodeMirror extension that renders a CSV column-header tooltip
 * on hover.
 *
 * Add this to the extension array for CSV text mode alongside
 * `csvRainbowHighlight` — no other wiring required.
 */
export const csvCellHeaderTooltip: Extension = [
    csvMetaField,
    hoverTooltip((view, pos, _side) => {
        const meta = view.state.field(csvMetaField);
        if (!meta) return null;

        const { delimiter, headers, headerLineNumber } = meta;

        const line = view.state.doc.lineAt(pos);

        // Suppress the tooltip on the header row itself — showing "Col 1: name"
        // while hovering *on* "name" in the header would be circular.
        if (line.number === headerLineNumber) return null;

        // Character offset of the hover position relative to the line start.
        const linePos = pos - line.from;

        const fields = getFieldRanges(line.text, delimiter);

        // Determine which field (if any) contains the hover position.
        // `to` in FieldRange is exclusive; a cursor exactly at `to` sits on
        // the delimiter character — do not associate it with that field.
        let colIndex = -1;
        for (let i = 0; i < fields.length; i++) {
            const { from, to } = fields[i];
            if (linePos >= from && linePos < to) {
                colIndex = i;
                break;
            }
        }

        // Cursor is on a delimiter, trailing whitespace, or an empty line.
        if (colIndex === -1) return null;

        // Column numbers are 1-based to match the VS Code / Rainbow CSV
        // convention users already expect.
        const colNum = colIndex + 1;
        const headerText = (headers[colIndex] ?? "").trim();
        const hasHeader = headerText.length > 0;

        // The clipboard target is the decoded header name when available, or
        // the positional label ("Col N") when the header is blank/missing, so
        // clicking the tooltip always does something useful.
        const copyTarget = hasHeader ? headerText : `Col ${colNum}`;

        const { from: fieldFrom, to: fieldTo } = fields[colIndex];

        return {
            pos: line.from + fieldFrom,
            end: line.from + fieldTo,
            above: true,
            strictSide: false,
            create() {
                const dom = document.createElement("div");
                dom.className = "cm-csv-header-tooltip";

                // "Col N" / "Col N: " prefix — always present
                const prefixSpan = document.createElement("span");
                prefixSpan.className = "cm-csv-col-prefix";
                prefixSpan.textContent = hasHeader
                    ? `Col ${colNum}: `
                    : `Col ${colNum}`;
                dom.appendChild(prefixSpan);

                // Header name — present only when the header cell is non-blank
                if (hasHeader) {
                    const nameSpan = document.createElement("span");
                    nameSpan.className = "cm-csv-col-name";
                    nameSpan.textContent = headerText;
                    dom.appendChild(nameSpan);
                }

                dom.addEventListener("click", () => {
                    navigator.clipboard
                        .writeText(copyTarget)
                        .then(() => toast.success("Copied column name to clipboard"))
                        .catch(() => toast.error("Failed to copy column name"));
                });

                return { dom };
            },
        };
    }),
];
