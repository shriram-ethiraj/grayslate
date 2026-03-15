import { hoverTooltip } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import type { EditorView } from "@codemirror/view";
import type { SyntaxNode } from "@lezer/common";
import { toast } from "$lib/components/ui/sonner";

/**
 * Extracts the unquoted key string from a `Property` node by reading its
 * first child (which is always the `PropertyName` node in the lezer-json grammar).
 */
export function extractPropertyKey(view: EditorView, propertyNode: SyntaxNode): string | null {
    const keyNode = propertyNode.firstChild;
    // lezer-json grammar names object keys "PropertyName", not "String".
    // "String" is reserved for string *values* only.
    if (!keyNode || keyNode.name !== "PropertyName") return null;
    const raw = view.state.sliceDoc(keyNode.from, keyNode.to);
    // A valid PropertyName token is always a quoted string (minimum `""` = 2 chars).
    // Returning null for anything shorter guards against a malformed/partial parse.
    return raw.length >= 2 ? raw.slice(1, -1) : null;
}

/**
 * Builds the full JSONPath-style key path (rooted at `$`) for any position
 * in the document.
 *
 * Algorithm: walk UP the lezer syntax tree from `pos`, prepending segments:
 *   - Current node OR its parent is `Property` → extract the key name
 *   - Parent is `Array`                        → count element index, prepend `[n]`
 *   - Otherwise                                → skip upwards
 *
 * Two cases handled for Property:
 *   a) `parent.name === "Property"`: cursor landed on a named child of Property
 *      (the key String, or value node).
 *   b) `current.name === "Property"`: cursor landed on an anonymous token inside
 *      Property (the `:` colon has no named lezer node, so `resolveInner` returns
 *      the enclosing Property node directly). In this case `parent.name` is
 *      `"Object"`, which would otherwise fall into the skip branch and lose the key.
 *
 * Examples:
 *   [{"id": 1, "address": {"city": "NY"}}]
 *   cursor on `"NY"`  →  "$[0].address.city"
 *   cursor on `"id"`   →  "$[0].id"
 *   cursor on `:` after `"id"` →  "$[0].id"   (was "$[0]" before this fix)
 *   cursor on `{` of first object  →  "$[0]"
 *   {"users": [{"name": "Alice"}]}
 *   cursor on `"Alice"`  →  "$.users[0].name"
 */
export function buildJsonPath(view: EditorView, pos: number, side: -1 | 1): string | null {
    const tree = syntaxTree(view.state);
    // Use the side value from CodeMirror (left/right half of character) so that
    // resolveInner snaps to the correct node at every cursor position.
    let current: SyntaxNode | null = tree.resolveInner(pos, side);

    // Segments are collected bottom-up (innermost → outermost) and reversed once
    // at the end — O(n) total vs. the O(n²) cost of repeated unshift.
    const parts: string[] = [];

    while (current !== null) {
        const parent: SyntaxNode | null = current.parent;
        if (parent === null) break;

        if (current.name === "Property") {
            // Case (b): `current` IS the Property node.
            // This happens when the cursor lands on the anonymous `:` token —
            // lezer has no named node for `:`, so resolveInner returns Property.
            const key = extractPropertyKey(view, current);
            if (key !== null) parts.push(key);
            // Advance past this Property; its parent is the enclosing Object.
            current = parent;
        } else if (parent.name === "Property") {
            // Case (a): `current` is a named child of Property
            // (the key PropertyName node, or the value node).
            const key = extractPropertyKey(view, parent);
            if (key !== null) parts.push(key);
            // Skip Property AND its enclosing Object in one step.
            current = parent.parent;
        } else if (parent.name === "Array") {
            // Count non-structural siblings before `current` to derive the 0-based index.
            let index = 0;
            let sibling: SyntaxNode | null = parent.firstChild;
            while (sibling !== null && sibling.from !== current.from) {
                const n = sibling.name;
                if (n !== "[" && n !== "]" && n !== "," && n !== "⚠") {
                    index++;
                }
                sibling = sibling.nextSibling;
            }
            parts.push(`[${index}]`);
            current = parent;
        } else {
            // Object, JsonText, or punctuation tokens — walk upward without adding a segment.
            current = parent;
        }
    }

    // Reverse to convert bottom-up collection order into top-down path order.
    parts.reverse();

    // Assemble the JSONPath string, always prefixed with the root `$`.
    // e.g. parts = ["users", "[0]", "address"] → "$.users[0].address"
    //      parts = ["[0]", "id"]               → "$[0].id"
    //      parts = []                           → "$"  ()
    return parts.reduce(
        (acc, part) => acc + (part.startsWith("[") ? part : "." + part),
        "$",
    );
}

/**
 * CodeMirror `hoverTooltip` extension that shows the full JSONPath key path
 * (e.g. `$[0].address.city`) when hovering over a **property key** in a JSON
 * document.  Hovering over values, braces, brackets, or other tokens does
 * nothing — only `PropertyName` nodes trigger the tooltip.
 *
 * Register this alongside the `json()` language extension — it has no effect
 * on non-JSON syntax trees.
 */
export const jsonKeyPath = hoverTooltip((view, pos, side) => {
    const tree = syntaxTree(view.state);
    const node = tree.resolveInner(pos, side);

    // Only show the tooltip when hovering over a property key (PropertyName),
    // not on values, braces, brackets, colons, or other tokens.
    if (node.name !== "PropertyName") return null;

    const path = buildJsonPath(view, pos, side);
    if (!path || path === "$") return null;

    return {
        pos: node.from,
        end: node.to,
        above: true,
        strictSide: false,
        create() {
            const dom = document.createElement("div");
            dom.className = "cm-json-key-path-tooltip";

            const textSpan = document.createElement("span");
            textSpan.textContent = path;
            dom.appendChild(textSpan);

            dom.addEventListener("click", () => {
                navigator.clipboard.writeText(path).then(() => {
                    toast.success("Copied path to clipboard");
                }).catch((err) => {
                    console.error("Failed to copy path:", err);
                    toast.error("Failed to copy path");
                });
            });

            return { dom };
        },
    };
});
