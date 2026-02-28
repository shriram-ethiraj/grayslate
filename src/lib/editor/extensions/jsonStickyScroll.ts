/**
 * jsonStickyScroll.ts
 *
 * JSON scope provider for the generic `stickyScroll` extension.
 *
 * Walks the lezer JSON syntax tree and returns a `StickyScope[]` with one
 * entry per multi-line Object or Array node.  The generic engine handles
 * scroll tracking, syntax-highlighted rendering, and the panel lifecycle.
 *
 * To add sticky scroll for another language (Markdown, XML, YAML, …),
 * create a similar thin module that provides a `computeScopes` function
 * and calls `createStickyScroll`.
 */

import { syntaxTree } from "@codemirror/language";
import type { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { createStickyScroll } from "./stickyScroll";
import type { StickyScope } from "./stickyScroll";

// ---------------------------------------------------------------------------
// Scope parsing
// ---------------------------------------------------------------------------

/**
 * Walk the lezer JSON syntax tree and build a flat, document-order list of
 * scopes.  Only Object and Array nodes that span more than one line are
 * included — single-line `{}` / `[]` are irrelevant for sticky scroll.
 */
export function parseJsonScopes(view: EditorView): StickyScope[] {
    const tree = syntaxTree(view.state);
    const doc = view.state.doc;
    const scopes: StickyScope[] = [];

    tree.iterate({
        enter(node) {
            if (node.name !== "Object" && node.name !== "Array") return;

            const openLine = doc.lineAt(node.from);
            const closeLine = doc.lineAt(node.to);

            // Single-line scope — skip (nothing to "stick")
            if (openLine.number === closeLine.number) return;

            scopes.push({
                openLine: openLine.number,
                closeLine: closeLine.number,
            });
        },
    });

    return scopes;
}

// ---------------------------------------------------------------------------
// Extension
// ---------------------------------------------------------------------------

/**
 * JSON sticky scroll extension — drop into any JSON extension array.
 *
 * Includes both the panel and the base theme (no separate theme import
 * needed).
 *
 * ```ts
 * import { jsonStickyScroll } from "./jsonStickyScroll";
 *
 * // In languageExtensions.ts:
 * case "json":
 *     return [json(), jsonStickyScroll, ...];
 * ```
 */
export const jsonStickyScroll: Extension = createStickyScroll({
    class: "cm-json-sticky-scroll",
    computeScopes: parseJsonScopes,
});

