/**
 * contextMenuExtension.ts
 *
 * CodeMirror 6 extension that handles global generic right-click behavior.
 * Prevents native right-click selection and moves the cursor to the click
 * location if clicking outside an existing selection.
 * Also extracts syntax-specific metadata (like JSON path/key/value) if applicable,
 * which is exposed via `consumeContextMenuData()`.
 */

import { EditorView } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import { buildJsonPath, extractPropertyKey } from "./jsonKeyPath";

export interface ContextMenuData {
    path?: string;
    key?: string;
    value?: string;
}

let _pending: ContextMenuData | null = null;

export function consumeContextMenuData(): ContextMenuData | null {
    const d = _pending;
    _pending = null;
    return d;
}

const WHITESPACE_RE = /\s/;

const VALID_JSON_NODE_NAMES = new Set([
    "PropertyName",
    "String",
    "Number",
    "Boolean",
    "Null",
    "Object",
    "Array",
    "Property",
]);

// ── Right-click state shared between pointerdown → contextmenu ──────────
// Between the two events, WebKit on macOS can apply native word / line
// selection on contenteditable.  We store the desired position early and
// re-assert it in contextmenu to undo any browser interference.
let _rightClickPos: number | null = null;
let _rightClickHadSelection: { from: number; to: number } | null = null;

// Block native `selectstart` events during right-click so WebKit never
// initiates word / line selection between pointerdown and contextmenu.
function blockSelectStart(e: Event) {
    e.preventDefault();
}

export const contextMenuExtension = EditorView.domEventHandlers({
    pointerdown(e, view) {
        // Handle right click (button 2) and macOS Ctrl+Click (button 0 + ctrlKey).
        if (e.button === 2 || (e.button === 0 && e.ctrlKey)) {
            e.preventDefault();
            if (!view.hasFocus) view.focus();

            const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
            if (pos !== null) {
                const { main } = view.state.selection;
                const isClickInsideSelection =
                    !main.empty && pos >= main.from && pos <= main.to;

                if (isClickInsideSelection) {
                    // Preserve the existing selection — store it so contextmenu
                    // can restore it after any browser interference.
                    _rightClickPos = null;
                    _rightClickHadSelection = { from: main.from, to: main.to };
                } else {
                    // Place the cursor and remember the position.
                    _rightClickPos = pos;
                    _rightClickHadSelection = null;
                    view.dispatch({
                        selection: { anchor: pos },
                        scrollIntoView: false,
                    });
                }
            } else {
                _rightClickPos = null;
                _rightClickHadSelection = null;
            }

            // Temporarily block selectstart to stop WebKit from applying
            // native word / line selection before contextmenu fires.
            view.dom.addEventListener("selectstart", blockSelectStart, true);

            return true; // Tells CodeMirror to skip its internal pointerdown handler
        }
    },
    mousedown(e) {
        // Defensive fallback: on non-standard WebViews that fire mousedown
        // even after pointerdown.preventDefault(), suppress CM's built-in
        // handler and prevent any additional browser default for button 2.
        if (e.button === 2 || (e.button === 0 && e.ctrlKey)) {
            e.preventDefault();
            return true;
        }
    },
    contextmenu(e: MouseEvent, view: EditorView) {
        _pending = null;

        // Remove the selectstart blocker — no longer needed after this event.
        view.dom.removeEventListener("selectstart", blockSelectStart, true);

        // Re-assert the cursor / selection from pointerdown.  Between
        // pointerdown and now, WebKit may have overwritten it with native
        // word or line selection on the contenteditable element.
        if (_rightClickPos !== null) {
            const { main } = view.state.selection;
            // Only dispatch if the selection actually drifted.
            if (main.empty ? main.from !== _rightClickPos : true) {
                view.dispatch({
                    selection: { anchor: _rightClickPos },
                    scrollIntoView: false,
                });
            }
        } else if (_rightClickHadSelection !== null) {
            const { main } = view.state.selection;
            const saved = _rightClickHadSelection;
            if (main.from !== saved.from || main.to !== saved.to) {
                view.dispatch({
                    selection: { anchor: saved.from, head: saved.to },
                    scrollIntoView: false,
                });
            }
        }

        // Resolve position for JSON metadata (use saved value or fresh coords).
        const pos =
            _rightClickPos ??
            view.posAtCoords({ x: e.clientX, y: e.clientY });

        // Clean up right-click state.
        _rightClickPos = null;
        _rightClickHadSelection = null;

        if (pos === null) {
            if (!view.hasFocus) view.focus();
            return false;
        }

        if (!view.hasFocus) view.focus();

        const tree = syntaxTree(view.state);

        // Check if we are inside a JSON document, otherwise we just show standard menu
        if (tree.topNode.name !== "JsonText") {
            return false;
        }

        // For bare whitespace that isn't inside a JSON string, skip JSON-
        // specific data but let the generic context menu appear.
        const charAtPos = view.state.sliceDoc(pos, pos + 1);
        if (!charAtPos || WHITESPACE_RE.test(charAtPos)) {
            const innerNode = tree.resolveInner(pos);
            if (
                innerNode.name !== "String" &&
                innerNode.name !== "PropertyName"
            ) {
                return false;
            }
            if (pos < innerNode.from || pos >= innerNode.to) {
                return false;
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
            !VALID_JSON_NODE_NAMES.has(targetNode.name) &&
            targetNode.name !== "JsonText"
        ) {
            return false;
        }

        const path = buildJsonPath(view, pos, -1);
        if (!path || path === "$") {
            return false;
        }

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
    }
});
