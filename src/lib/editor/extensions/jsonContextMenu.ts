/**
 * jsonContextMenu.ts
 *
 * CodeMirror 6 extension that intercepts `contextmenu` events inside JSON
 * documents, performs syntax-tree hit-testing, and stores the resolved
 * path / key / value for the companion `JsonContextMenu.svelte` component.
 *
 * Register this extension ONLY for JSON via `getLanguageExtension`.
 *
 * Communication with Svelte
 * -------------------------
 * The extension stores its result in a module-level variable (`_pending`).
 * Because CM's `domEventHandlers` listener on `view.dom` is registered
 * during EditorView construction (i.e. before any later `addEventListener`
 * call in a Svelte `$effect`), the CM handler always fires first for the
 * same `contextmenu` event.  The Svelte component then reads the stored
 * data synchronously via `consumeJsonContextMenuData()`.
 */

import { EditorView } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import { buildJsonPath, extractPropertyKey } from "./jsonKeyPath";

// ─── Public Data Contract ────────────────────────────────────────────────────

/** Data resolved by the extension for a valid right-click on a JSON node. */
export interface JsonContextMenuData {
    path: string;
    key: string;
    value: string;
}

// ─── Module-Level Pending Data ───────────────────────────────────────────────

let _pending: JsonContextMenuData | null = null;

/**
 * Read **and clear** the pending context-menu data.
 * Returns `null` when the last right-click did not land on a valid JSON node
 * (or when no JSON extension is active at all).
 */
export function consumeJsonContextMenuData(): JsonContextMenuData | null {
    const d = _pending;
    _pending = null;
    return d;
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

const WHITESPACE_RE = /\s/;

/** JSON syntax-node names that qualify as right-click targets. */
const VALID_NODE_NAMES = new Set([
    "PropertyName",
    "String",
    "Number",
    "Boolean",
    "Null",
    "Object",
    "Array",
    "Property",
]);

// ─── Extension ───────────────────────────────────────────────────────────────

/**
 * CodeMirror extension that:
 *   1. Prevents native right-click word-selection (pointerdown / mousedown).
 *   2. On `contextmenu`, hit-tests the JSON syntax tree.
 *      • Valid node  → stores path/key/value, lets the event propagate.
 *      • Invalid pos → calls `e.preventDefault()` (suppresses native menu)
 *        and returns `true` so CM also calls `preventDefault`.
 */
export const jsonContextMenuExtension = EditorView.domEventHandlers({
    // ── Prevent native right-click word-selection ──────────────────────────
    pointerdown(e) {
        if (e.button === 2) {
            e.preventDefault();
            return true;
        }
    },
    mousedown(e) {
        if (e.button === 2) {
            e.preventDefault();
            return true;
        }
    },

    // ── Main contextmenu handler ──────────────────────────────────────────
    contextmenu(e: MouseEvent, view: EditorView) {
        _pending = null;

        // posAtCoords returns null for clicks outside any character
        // (gutter, empty padding below the last line, etc.).
        const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
        if (pos === null) {
            e.preventDefault();
            return true;
        }

        const tree = syntaxTree(view.state);

        // Suppress for bare whitespace that isn't inside a JSON string.
        const charAtPos = view.state.sliceDoc(pos, pos + 1);
        if (!charAtPos || WHITESPACE_RE.test(charAtPos)) {
            const innerNode = tree.resolveInner(pos);
            if (
                innerNode.name !== "String" &&
                innerNode.name !== "PropertyName"
            ) {
                e.preventDefault();
                return true;
            }
            if (pos < innerNode.from || pos >= innerNode.to) {
                e.preventDefault();
                return true;
            }
        }

        // Walk up from structural punctuation to the meaningful parent.
        let node = tree.resolveInner(pos, -1);
        let targetNode = node;
        if (
            ["{", "}", "[", "]", ":", ","].includes(node.name) &&
            node.parent
        ) {
            targetNode = node.parent;
        }

        if (
            !VALID_NODE_NAMES.has(targetNode.name) &&
            targetNode.name !== "JsonText"
        ) {
            e.preventDefault();
            return true;
        }

        const path = buildJsonPath(view, pos, -1);
        if (!path || path === "$") {
            e.preventDefault();
            return true;
        }

        // ── Move cursor to click position ──────────────────────────────────
        // Anchor-only selection avoids highlightSelectionMatches highlights.
        view.dispatch({
            selection: { anchor: pos },
            scrollIntoView: false,
        });

        // ── Resolve the raw value text ─────────────────────────────────────
        let valueToCopy = "";

        if (
            targetNode.name === "PropertyName" ||
            (targetNode.name === "Property" &&
                targetNode.firstChild?.name === "PropertyName")
        ) {
            const propNode =
                targetNode.name === "PropertyName"
                    ? targetNode.parent
                    : targetNode;
            if (propNode?.name === "Property") {
                const valNode = propNode.lastChild;
                if (valNode) {
                    valueToCopy = view.state.sliceDoc(valNode.from, valNode.to);
                }
            }
        } else {
            valueToCopy = view.state.sliceDoc(targetNode.from, targetNode.to);
        }

        // Pretty-print / unwrap the value for clipboard display.
        try {
            if (
                targetNode.name === "Object" ||
                targetNode.name === "Array" ||
                targetNode.name === "Property"
            ) {
                valueToCopy = JSON.stringify(JSON.parse(valueToCopy), null, 2);
            } else if (
                targetNode.name === "String" ||
                targetNode.name === "PropertyName"
            ) {
                const parsed = JSON.parse(valueToCopy);
                if (typeof parsed === "string") valueToCopy = parsed;
            } else {
                const parsed = JSON.parse(valueToCopy);
                if (typeof parsed !== "object") valueToCopy = String(parsed);
            }
        } catch {
            // keep raw sliceDoc text on parse failure
        }

        // ── Resolve the bare key name ──────────────────────────────────────
        let keyName = "";
        if (targetNode.name === "PropertyName") {
            keyName = extractPropertyKey(view, targetNode.parent!) ?? "";
        } else if (
            targetNode.name === "Property" ||
            (targetNode.name === "String" &&
                targetNode.parent?.name === "Property" &&
                targetNode.parent.firstChild === targetNode)
        ) {
            const propNode =
                targetNode.name === "Property"
                    ? targetNode
                    : targetNode.parent!;
            keyName = extractPropertyKey(view, propNode) ?? "";
        }

        // Store for the Svelte component and let the event propagate.
        _pending = { path, key: keyName, value: valueToCopy };
        return false;
    },
});
