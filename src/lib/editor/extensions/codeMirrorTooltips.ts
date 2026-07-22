import { ViewPlugin, type EditorView, type ViewUpdate } from "@codemirror/view";

function replaceNativeFoldTitles(view: EditorView): void {
    const markers = view.dom.querySelectorAll<HTMLElement>(".cm-foldGutter [title]");

    for (const marker of markers) {
        const content = marker.getAttribute("title");
        marker.removeAttribute("title");
        if (!content) continue;
        marker.dataset.cmTooltip = content;
        marker.setAttribute("aria-label", content);
    }
}

/**
 * CodeMirror owns the fold-gutter DOM and recreates its markers as the
 * viewport changes. Convert its native `title` labels after each relevant
 * update so the gutter uses the same fast, themed tooltip treatment as the
 * rest of the application.
 */
export const codeMirrorTooltips = ViewPlugin.fromClass(
    class {
        constructor(view: EditorView) {
            replaceNativeFoldTitles(view);
        }

        update(update: ViewUpdate): void {
            if (update.docChanged || update.viewportChanged || update.geometryChanged) {
                replaceNativeFoldTitles(update.view);
            }
        }
    },
);
