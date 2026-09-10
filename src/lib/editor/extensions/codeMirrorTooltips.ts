import {
    closeHoverTooltips,
    ViewPlugin,
    type EditorView,
    type ViewUpdate,
} from "@codemirror/view";
import {
    onTooltipWindowDeactivated,
    tooltipLifecycle,
} from "$lib/components/ui/tooltip/tooltip-lifecycle.svelte.js";

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
        private readonly stopWatchingDeactivation: () => void;
        private readonly view: EditorView;
        private readonly viewDom: HTMLElement;

        constructor(view: EditorView) {
            this.view = view;
            this.viewDom = view.dom;
            replaceNativeFoldTitles(view);
            if (!tooltipLifecycle.active) {
                view.dom.dataset.tooltipsSuspended = "true";
            }

            view.dom.addEventListener("pointermove", this.handlePointerMove, {
                capture: true,
                passive: true,
            });
            this.stopWatchingDeactivation = onTooltipWindowDeactivated(() => {
                view.dom.dataset.tooltipsSuspended = "true";

                // CodeMirror's hover plugins own private timers and async request
                // guards. Their normal mouseleave path cancels both; the public
                // effect then closes every active hover tooltip in one transaction.
                view.dom.dispatchEvent(
                    new MouseEvent("mouseleave", {
                        bubbles: false,
                        relatedTarget: null,
                    }),
                );
                view.dispatch({ effects: closeHoverTooltips });
            });
        }

        private readonly handlePointerMove = (): void => {
            if (!tooltipLifecycle.active) return;
            replaceNativeFoldTitles(this.view);
            delete this.viewDom.dataset.tooltipsSuspended;
        };

        update(update: ViewUpdate): void {
            if (update.docChanged || update.viewportChanged || update.geometryChanged) {
                replaceNativeFoldTitles(update.view);
            }
        }

        destroy(): void {
            this.stopWatchingDeactivation();
            this.viewDom.removeEventListener("pointermove", this.handlePointerMove, true);
            delete this.viewDom.dataset.tooltipsSuspended;
        }
    },
);
