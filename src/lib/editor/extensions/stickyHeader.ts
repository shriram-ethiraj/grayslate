/**
 * stickyHeader.ts
 *
 * A reusable CodeMirror 6 sticky-header panel that hides while the
 * "anchor line" is visible on-screen and slides into view once the user
 * scrolls past it — like VS Code's sticky scroll feature.
 *
 * Uses `showPanel` (top panel) + a passive native `scroll` listener on
 * `view.scrollDOM` for pixel-accurate, zero-layout-cost visibility toggling.
 *
 * The panel mirrors the editor gutter: an optional line-number element
 * occupies the exact gutter width (right-aligned, matching CM's own
 * `cm-lineNumbers` styling), and the consumer's content fills the rest.
 *
 * ### Performance notes
 *
 * | Operation               | Cost                                  |
 * |------------------------|---------------------------------------|
 * | Scroll handler          | `scrollTop` read (cached) + int compare |
 * | Visibility toggle       | single `display` property flip         |
 * | Content re-render       | only on `docChanged` + equality check  |
 * | Style sync              | `offsetLeft` + cached `getComputedStyle` reads; DOM writes only on change |
 *
 * The scroll listener is registered with `{ passive: true }` so it never
 * blocks the browser's scroll compositor.
 *
 * ### Usage
 *
 * ```ts
 * import { createStickyHeaderPanel } from "./stickyHeader";
 *
 * const panel = createStickyHeaderPanel({
 *     class: "csv-sticky-header",
 *     anchorLine: 1,
 *     getLineNumber: (view) => 1,
 *     render(dom, view) {
 *         dom.textContent = view.state.doc.line(1).text;
 *     },
 *     shouldRerender(update) {
 *         if (!update.docChanged) return false;
 *         return true;
 *     },
 * });
 * // panel is an Extension — pass it into your extension array.
 * ```
 */

import { showPanel, EditorView } from "@codemirror/view";
import type { Panel, ViewUpdate } from "@codemirror/view";
import type { Extension } from "@codemirror/state";

// ---------------------------------------------------------------------------
// Public config
// ---------------------------------------------------------------------------

export interface StickyHeaderConfig {
    /**
     * CSS class name applied to the panel's root `<div>`.
     * Consumers are responsible for styling via `EditorView.baseTheme()`
     * or external CSS.
     */
    class: string;

    /**
     * 1-based line number that acts as the visibility anchor.
     * The panel is hidden while this line is on-screen and shown
     * once the user scrolls past it.
     *
     * @default 1
     */
    anchorLine?: number;

    /**
     * Return the 1-based line number to display in the gutter area of the
     * sticky header.  Called on init and on every re-render.
     *
     * Return `null` to hide the line-number element (falls back to plain
     * padding).
     *
     * For CSV this is simply the anchor line (1).  For a JSON breadcrumb it
     * would be the line of the enclosing scope boundary.
     */
    getLineNumber?: (view: EditorView) => number | null;

    /**
     * Populate or update `dom` with the panel content.
     * Called once on init and again whenever `shouldRerender` returns `true`.
     *
     * `dom` is the **content container** (right of the line-number gutter),
     * not the panel root.  Direct DOM manipulation is preferred.
     */
    render(dom: HTMLElement, view: EditorView): void;

    /**
     * Return `true` when the panel content needs a re-render.
     * Called on every CM state update.  Keep it as cheap as possible
     * (e.g. a string equality check on the anchor line's text).
     *
     * @default () => false  (never re-renders after init)
     */
    shouldRerender?(update: ViewUpdate): boolean;
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/**
 * Check whether the anchor line is (partially) visible by comparing
 * `scrollTop` against the line's pixel-bottom from the CM height map.
 *
 * `lineBlockAt` reads a balanced-tree — O(log n), no DOM measurement.
 * `scrollTop` is a cached native property — no layout reflow.
 */
function isAnchorLineVisible(view: EditorView, anchorLine: number): boolean {
    const lineCount = view.state.doc.lines;
    if (anchorLine > lineCount) return false;
    const block = view.lineBlockAt(view.state.doc.line(anchorLine).from);
    return view.scrollDOM.scrollTop < block.bottom;
}

/**
 * Synchronise the sticky header's visual properties with the live editor,
 * pulling font, gutter width, and gutter colour directly from the DOM so
 * any theme change is automatically reflected.
 *
 * Reads:
 *   - `view.contentDOM` computed style   → font-family, font-size, line-height
 *   - `view.contentDOM.offsetLeft`        → gutter width (distance from scroller
 *     left edge to content left edge)
 *   - `.cm-content` padding-left          → typically 0 in CM6 base theme
 *   - `.cm-line` padding-left             → typically 6px in CM6 base theme
 *     (`padding: 0 2px 0 6px`).  This is where text actually starts.
 *   - `.cm-gutters` computed style        → color (line-number foreground)
 *
 * All values are compared against the previous frame's cache (`SyncState`)
 * so DOM writes only happen when something actually changed.
 */

/** Per-instance cache so we skip redundant DOM style writes. */
interface SyncState {
    fontKey: string;
    gutterWidth: number;
    textPad: number;
    gutterColor: string;
    editorBg: string;
    hoverBg: string;
}

function createSyncState(): SyncState {
    return {
        fontKey: "",
        gutterWidth: -1,
        textPad: -1,
        gutterColor: "",
        editorBg: "",
        hoverBg: "",
    };
}

function syncEditorStyles(
    lineNumEl: HTMLElement | null,
    contentEl: HTMLElement,
    root: HTMLElement,
    view: EditorView,
    state: SyncState,
): void {
    const cs = getComputedStyle(view.contentDOM);

    // ── Font (root) ──────────────────────────────────────────────────
    const fontFamily = cs.fontFamily;
    const fontSize = cs.fontSize;
    const lineHeight = cs.lineHeight;

    const fontKey = `${fontFamily}|${fontSize}|${lineHeight}`;
    if (fontKey !== state.fontKey) {
        state.fontKey = fontKey;
        root.style.fontFamily = fontFamily;
        root.style.fontSize = fontSize;
        root.style.lineHeight = lineHeight;
    }

    // ── Text-start alignment ────────────────────────────────────────
    // In CodeMirror 6 the actual character content begins at:
    //
    //   gutter width  (`.cm-content.offsetLeft` — distance from the
    //                  `.cm-scroller` padding edge to `.cm-content`)
    //   + `.cm-content` padding-left  (usually 0)
    //   + `.cm-line` padding-left     (usually 6 px — CM base theme)
    //
    // We read both paddings so any custom theme is handled correctly.
    const gutterWidth = view.contentDOM.offsetLeft;
    const contentPadding = parseFloat(cs.paddingLeft) || 0;

    // Read .cm-line padding — this is where text actually starts within
    // each line element.  Falls back to 0 when the document is empty.
    const lineEl = view.contentDOM.querySelector(".cm-line");
    const linePadding = lineEl
        ? parseFloat(getComputedStyle(lineEl).paddingLeft) || 0
        : 0;
    const textPad = contentPadding + linePadding;

    if (lineNumEl) {
        if (state.gutterWidth !== gutterWidth) {
            state.gutterWidth = gutterWidth;
            lineNumEl.style.width = `${gutterWidth}px`;
        }
        if (root.style.paddingLeft) root.style.paddingLeft = "";
        if (state.textPad !== textPad) {
            state.textPad = textPad;
            contentEl.style.paddingLeft = `${textPad}px`;
        }
    } else {
        const totalPad = gutterWidth + textPad;
        if (state.gutterWidth !== gutterWidth || state.textPad !== textPad) {
            state.gutterWidth = gutterWidth;
            state.textPad = textPad;
            root.style.paddingLeft = `${totalPad}px`;
        }
    }

    // ── Editor background — match the editor, not a separate colour ──
    // Use a CSS custom property instead of an inline style so the
    // :hover rule can override background-color without specificity fights.
    const editorBg = getComputedStyle(view.dom).backgroundColor;
    if (state.editorBg !== editorBg) {
        state.editorBg = editorBg;
        root.style.setProperty("--cm-sticky-bg", editorBg);
    }

    // ── Hover highlight — read .cm-activeLine background ─────────────
    // Falls back to a subtle semi-transparent overlay when the theme
    // doesn't define an activeLine background (or sets it transparent).
    const activeLineEl = view.dom.querySelector(".cm-activeLine");
    let hoverBg = "rgba(128, 128, 128, 0.08)";
    if (activeLineEl) {
        const alBg = getComputedStyle(activeLineEl).backgroundColor;
        if (alBg && alBg !== "rgba(0, 0, 0, 0)" && alBg !== "transparent") {
            hoverBg = alBg;
        }
    }
    if (state.hoverBg !== hoverBg) {
        state.hoverBg = hoverBg;
        root.style.setProperty("--cm-sticky-hover-bg", hoverBg);
    }

    // ── Line-number colour (read from .cm-gutters) ───────────────────
    if (lineNumEl) {
        const gutterEl = view.dom.querySelector(".cm-gutters");
        if (gutterEl) {
            const gutterColor = getComputedStyle(gutterEl).color;
            if (state.gutterColor !== gutterColor) {
                state.gutterColor = gutterColor;
                lineNumEl.style.color = gutterColor;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create a sticky-header `Extension` from the given config.
 *
 * Returns a `showPanel` extension that can be spread directly into a
 * CodeMirror extension array.
 */
export function createStickyHeaderPanel(config: StickyHeaderConfig): Extension {
    const anchor = config.anchorLine ?? 1;

    function factory(view: EditorView): Panel {
        // ── DOM structure ────────────────────────────────────────────
        // root.cm-sticky-header
        //   ├── span.cm-sticky-header-linenum   (optional, gutter-width)
        //   └── div.cm-sticky-header-content     (flex: 1)
        const root = document.createElement("div");
        root.className = `cm-sticky-header ${config.class}`;

        let lineNumEl: HTMLElement | null = null;
        if (config.getLineNumber) {
            lineNumEl = document.createElement("span");
            lineNumEl.className = "cm-sticky-header-linenum";
            root.appendChild(lineNumEl);
        }

        const contentEl = document.createElement("div");
        contentEl.className = "cm-sticky-header-content";
        root.appendChild(contentEl);

        // ── State ────────────────────────────────────────────────────
        const syncState = createSyncState();
        let visible = !isAnchorLineVisible(view, anchor);
        root.style.display = visible ? "" : "none";

        function renderAll(v: EditorView): void {
            // Line number
            if (lineNumEl && config.getLineNumber) {
                const ln = config.getLineNumber(v);
                lineNumEl.textContent = ln != null ? String(ln) : "";
            }
            // Consumer content
            config.render(contentEl, v);
        }

        // Initial style sync + content render.
        syncEditorStyles(lineNumEl, contentEl, root, view, syncState);
        renderAll(view);

        function syncVisibility(): void {
            const shouldShow = !isAnchorLineVisible(view, anchor);
            if (shouldShow !== visible) {
                visible = shouldShow;
                root.style.display = visible ? "" : "none";
            }
        }

        // ── Click-to-navigate ────────────────────────────────────────
        // Clicking the sticky header scrolls to and focuses the anchor
        // line, matching VS Code's sticky-scroll behaviour.
        function handleClick(): void {
            const lineNum = config.getLineNumber?.(view) ?? anchor;
            if (lineNum < 1 || lineNum > view.state.doc.lines) return;
            const pos = view.state.doc.line(lineNum).from;
            view.dispatch({
                selection: { anchor: pos },
                scrollIntoView: true,
            });
            view.focus();
        }
        root.addEventListener("click", handleClick);

        // Passive scroll listener for real-time visibility toggling.
        view.scrollDOM.addEventListener("scroll", syncVisibility, { passive: true });

        return {
            top: true,
            dom: root,
            update(update: ViewUpdate): void {
                // Visibility check on every state update (covers cursor
                // moves and other non-scroll interactions).
                syncVisibility();

                // Keep styles in sync (font from theme, gutter width
                // from line-count changes, etc.).
                syncEditorStyles(lineNumEl, contentEl, root, update.view, syncState);

                // Re-render content only when the consumer says so.
                if (config.shouldRerender?.(update)) {
                    renderAll(update.view);
                }
            },
            destroy(): void {
                view.scrollDOM.removeEventListener("scroll", syncVisibility);
                root.removeEventListener("click", handleClick);
            },
        };
    }

    return showPanel.of(factory);
}

// ---------------------------------------------------------------------------
// Base theme for common sticky-header styling
// ---------------------------------------------------------------------------

/**
 * Shared base styles for any `cm-sticky-header` panel.
 * Individual consumers add their own class for feature-specific styling.
 *
 * The root is a flex row.  The optional line-number span occupies the
 * gutter width (right-aligned, muted colour to match CM's line numbers).
 * The content div takes the remaining space.
 */
export const stickyHeaderBaseTheme: Extension = EditorView.baseTheme({
    // Font family, size, line-height, and gutter colour are synced from
    // the live editor DOM at runtime (see syncEditorStyles), so they
    // adapt automatically when the user switches themes.
    ".cm-sticky-header": {
        display: "flex",
        alignItems: "baseline",
        whiteSpace: "nowrap",
        overflow: "hidden",
        // --cm-sticky-bg is set at runtime from the editor background.
        // --cm-sticky-hover-bg is set at runtime from .cm-activeLine.
        // Using custom properties (not inline styles) ensures :hover
        // can override background-color without specificity conflicts.
        backgroundColor: "var(--cm-sticky-bg, transparent)",
        borderBottom: "1px solid color-mix(in srgb, currentColor 12%, transparent)",
        boxSizing: "border-box",
        cursor: "pointer",
    },
    ".cm-sticky-header:hover": {
        backgroundColor: "var(--cm-sticky-hover-bg, rgba(128,128,128,.08))",
    },
    // Line-number element — width and colour are set at runtime
    ".cm-sticky-header-linenum": {
        display: "inline-block",
        boxSizing: "border-box",
        textAlign: "right",
        paddingRight: "8px",
        paddingLeft: "5px",
        flexShrink: "0",
        userSelect: "none",
    },
    // Content container
    ".cm-sticky-header-content": {
        flex: "1",
        overflow: "hidden",
        textOverflow: "ellipsis",
    },
});
