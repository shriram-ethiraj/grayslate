/**
 * jsonFoldWidget.ts
 *
 * A CodeMirror extension that replaces the default fold placeholder (…) with
 * a context-aware widget that shows useful info about the collapsed node:
 *
 * Examples:
 *   Array  → [… 4]                              (item count)
 *   Object → { id: 1, name: "Alice", … }        (first few key: value pairs)
 *   Other  → …                                  (fallback)
 */

import { codeFolding, syntaxTree } from "@codemirror/language";
import { Prec } from "@codemirror/state";
import type { EditorState } from "@codemirror/state";
import { hoverTooltip, type EditorView } from "@codemirror/view";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface KeyValuePair {
    key: string;
    value: string;
}

type FoldInfo =
    | { type: "array"; count: number }
    | { type: "object"; preview: KeyValuePair[]; total: number }
    | { type: "other" };

/**
 * All node names in the lezer-json grammar that represent a JSON value.
 * Used to count direct children of an Array node.
 */
const JSON_VALUE_TYPES = new Set([
    "Object",
    "Array",
    "String",
    "Number",
    "True",
    "False",
    "Null",
]);

/** Maximum number of key-value pairs to preview in an object placeholder. */
const MAX_PREVIEW_PAIRS = 2;

/**
 * Hard limit on how many children to scan when building the fold placeholder.
 * Prevents main-thread freezes on files with gigabytes of data in a single array/object.
 */
const MAX_SCAN_CHILDREN = 100;

/** Maximum characters for a single value before truncating with "…". */
const MAX_VALUE_LEN = 20;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Truncates a string to MAX_VALUE_LEN, appending "…" if cut.
 */
function truncate(s: string): string {
    return s.length > MAX_VALUE_LEN ? s.slice(0, MAX_VALUE_LEN) + "…" : s;
}

/**
 * Formats a JSON value node as a compact, readable string.
 * Nested objects/arrays are shown as {…} / […] rather than expanded.
 */
function formatValue(state: EditorState, from: number, to: number, nodeName: string): string {
    switch (nodeName) {
        case "Object":
            return "{…}";
        case "Array":
            return "[…]";
        case "String": {
            // Include the quotes but truncate long strings.
            const raw = state.sliceDoc(from, to);
            return truncate(raw);
        }
        default:
            // Number, True, False, Null — always short
            return state.sliceDoc(from, to);
    }
}

// ---------------------------------------------------------------------------
// Fold helpers
// ---------------------------------------------------------------------------

/**
 * Walks the syntax tree to find the Array or Object node that owns the fold
 * range, then extracts count / preview pairs.
 *
 * Called once per fold (during placeholder creation), so it is cheap.
 */
function preparePlaceholder(
    state: EditorState,
    range: { from: number; to: number },
): FoldInfo {
    const tree = syntaxTree(state);

    // Resolve one character inside the opening bracket to land on the
    // container node (Array/Object) rather than the bracket token itself.
    let node = tree.resolveInner(range.from + 1, 1);

    // Walk up in case we started on a nested leaf.
    while (node && node.name !== "Array" && node.name !== "Object") {
        if (!node.parent) break;
        node = node.parent;
    }

    if (node.name === "Array") {
        // Count direct children that are JSON values (skip brackets/commas).
        let count = 0;
        let scanned = 0;
        let child = node.firstChild;
        while (child && scanned < MAX_SCAN_CHILDREN) {
            if (JSON_VALUE_TYPES.has(child.name)) count++;
            scanned++;
            child = child.nextSibling;
        }
        return { type: "array", count: scanned >= MAX_SCAN_CHILDREN ? Infinity : count };
    }

    if (node.name === "Object") {
        const preview: KeyValuePair[] = [];
        let total = 0;
        let scanned = 0;
        let child = node.firstChild;

        while (child && scanned < MAX_SCAN_CHILDREN) {
            if (child.name === "Property") {
                total++;
                if (preview.length < MAX_PREVIEW_PAIRS) {
                    // Property → String ":" <value>
                    const keyNode = child.firstChild; // String node (quoted key)
                    if (keyNode) {
                        // Strip surrounding quotes from the key.
                        const rawKey = state.sliceDoc(keyNode.from, keyNode.to);
                        const key = rawKey.replace(/^"|"$/g, "");

                        // The value is the last child of Property.
                        const valueNode = child.lastChild;
                        const value = valueNode
                            ? formatValue(state, valueNode.from, valueNode.to, valueNode.name)
                            : "…";

                        preview.push({ key, value });
                    }
                }
            }
            scanned++;
            child = child.nextSibling;
        }

        return { type: "object", preview, total: scanned >= MAX_SCAN_CHILDREN ? Infinity : total };
    }

    return { type: "other" };
}

/**
 * Builds the DOM element shown in place of the folded text.
 * Reuses `.cm-foldPlaceholder` for consistent base styling.
 */
function placeholderDOM(
    _view: EditorView,
    onclick: (event: Event) => void,
    prepared: unknown,
): HTMLElement {
    const info = prepared as FoldInfo;

    const span = document.createElement("span");
    span.setAttribute("role", "button");
    span.setAttribute("aria-label", "folded code");
    span.className = "cm-foldPlaceholder";
    span.addEventListener("click", onclick);

    let tooltipText: string;

    if (info.type === "array") {
        const isCapped = info.count === Infinity;
        const displayCount = isCapped ? `${MAX_SCAN_CHILDREN}+` : info.count;
        const label = info.count === 1 ? "1 item" : `${displayCount} items`;

        span.textContent = `… ${displayCount}`;
        tooltipText = `Click to unfold (${label})`;

    } else if (info.type === "object") {
        const { preview, total } = info;
        const isCapped = total === Infinity;
        const displayTotal = isCapped ? `${MAX_SCAN_CHILDREN}+` : total;

        // hasMore is true if we stopped extracting preview pairs, or if we hit the limit
        const hasMore = preview.length === MAX_PREVIEW_PAIRS || isCapped;

        // The editor already renders the surrounding { } braces — we only
        // provide the inner summary so the result reads as  { key: val, … }
        // without doubling up the braces.
        const pairs = preview.map(({ key, value }) => `${key}: ${value}`).join(", ");
        span.textContent = ` ${pairs}${hasMore ? ", …" : " "}`;
        tooltipText = `Click to unfold (${displayTotal} ${total === 1 ? "key" : "keys"})`;

    } else {
        // Fallback for non-JSON fold targets.
        span.textContent = "…";
        tooltipText = "Click to unfold";
    }

    span.dataset.tooltip = tooltipText;
    span.setAttribute("aria-label", tooltipText);

    return span;
}

const jsonFoldTooltip = hoverTooltip(
    (view, pos) => {
        const domAtPosition = view.domAtPos(pos);
        const element = domAtPosition.node instanceof Element
            ? domAtPosition.node
            : domAtPosition.node.parentElement;
        const placeholder = element?.closest<HTMLElement>(".cm-foldPlaceholder[data-tooltip]");

        if (!placeholder || !view.dom.contains(placeholder)) return null;

        const content = placeholder.dataset.tooltip;
        if (!content) return null;

        return {
            pos,
            above: true,
            create() {
                const dom = document.createElement("div");
                dom.className = "cm-json-fold-tooltip";
                dom.textContent = content;
                return { dom };
            },
        };
    },
    { hoverTime: 250 },
);

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/**
 * Drop-in CodeMirror extension.
 *
 * Uses `Prec.highest` so it wins over the default `codeFolding()` that
 * `basicSetup` installs, since `combineConfig` takes the first-defined value
 * for each option key.
 */
export const jsonFoldWidget = [
    Prec.highest(codeFolding({ preparePlaceholder, placeholderDOM })),
    jsonFoldTooltip,
];
