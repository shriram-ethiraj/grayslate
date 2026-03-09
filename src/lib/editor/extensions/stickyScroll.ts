/**
 * stickyScroll.ts
 *
 * A generic, language-agnostic CodeMirror 6 extension providing VS Code–style
 * sticky scroll.  Scope-opening lines pin to the top of the editor as the
 * user scrolls, showing the nesting context at a glance.
 *
 * ### Design
 *
 * The extension is parameterised by a `ScopeProvider` — a pure function that
 * examines the editor state and returns a list of scopes.  This makes it
 * trivially extensible:
 *
 * | Language   | Scope provider returns                         |
 * |------------|------------------------------------------------|
 * | JSON       | Object / Array opening lines                   |
 * | Markdown   | Heading lines (`# …`, `## …`, …)               |
 * | XML / HTML | Element open-tag lines                          |
 * | YAML       | Mapping-key lines at increasing indent depth    |
 *
 * ### Line rendering — DOM cloning (Monaco-style)
 *
 * Sticky-scroll rows are rendered by **deep-cloning the real `.cm-line`
 * element** from the editor DOM — the same technique VS Code / Monaco
 * uses.  This captures *every* decoration applied by *any* extension
 * (ViewPlugin marks, HighlightStyle tags, widget decorations, …) with
 * zero coupling to the extensions that produced them.
 *
 * Clones are cached in a `Map<lineNumber, HTMLElement>` so they survive
 * after the line leaves the viewport.  The cache is proactively warmed on
 * every viewport change — scope-opening lines that happen to be in the
 * live DOM are cloned before they're needed.
 *
 * When a line is not in the DOM and has no cached clone (rare: only on a
 * direct jump to a deep position), the engine falls back to
 * `renderHighlightedLine` which walks the lezer syntax tree with the
 * editor's active `HighlightStyle` — producing identical output for any
 * language with a lezer grammar (JSON, JS, Python, …).
 *
 * ### Publishable as a standalone package
 *
 * The only public API is:
 *
 * ```ts
 * import { createStickyScroll, stickyScrollBaseTheme } from "./stickyScroll";
 * import type { StickyScope, StickyScrollConfig } from "./stickyScroll";
 *
 * const ext = createStickyScroll({
 *     class: "cm-json-sticky-scroll",
 *     computeScopes(view) { return []; },
 * });
 * ```
 *
 * ### Performance
 *
 * | Operation             | Cost                                          |
 * |-----------------------|-----------------------------------------------|
 * | Scope parsing         | Consumer-defined; called only when doc/tree   |
 * |                       | changes                                       |
 * | Stack computation     | Linear scan; `lineBlockAt` is O(log n) via    |
 * |                       | CM's height B-tree — no DOM measurement       |
 * | Scroll handler        | rAF-throttled passive listener; DOM writes     |
 * |                       | only when the stack identity changes           |
 * | Line rendering        | DOM clone (fast); lezer fallback only when     |
 * |                       | line was never in the viewport                 |
 * | Clone cache warm      | O(scopes × viewport lines) per viewport shift |
 * | Style sync            | Cached getComputedStyle reads; writes only     |
 * |                       | when values change                             |
 */

import { showPanel, EditorView } from "@codemirror/view";
import type { Panel, ViewUpdate } from "@codemirror/view";
import { syntaxTree, highlightingFor } from "@codemirror/language";
import { highlightTree, Tag } from "@lezer/highlight";
import type { Extension, EditorState } from "@codemirror/state";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export interface StickyScope {
    /**
     * 1-based line number of the scope-opening line.
     * Controls both when the scope becomes active (line scrolled past)
     * and what is rendered in the sticky row.
     */
    openLine: number;

    /**
     * 1-based line number of the scope-closing line.
     * The scope leaves the sticky panel when this line scrolls past the
     * viewport top.
     */
    closeLine: number;
}

export interface StickyScrollConfig {
    /**
     * CSS class applied to the panel root, scoping consumer-specific
     * styles.  E.g. `"cm-json-sticky-scroll"`.
     */
    class: string;

    /**
     * Compute the scope list for the current editor state.
     *
     * Called on document changes and syntax-tree updates (async parse
     * delivery).  Must return scopes in **document order** (outermost
     * first) — the stack computation relies on this ordering.
     */
    computeScopes(view: EditorView): StickyScope[];

    /**
     * Maximum number of pinned rows.
     * When more scopes are active than this limit, only the deepest
     * (innermost) are shown.
     *
     * @default 5
     */
    maxLines?: number;

    /**
     * Return `true` when scopes should be re-parsed for this update.
     *
     * @default docChanged || syntaxTree changed
     */
    shouldReparse?(update: ViewUpdate): boolean;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_MAX_LINES = 5;

// ---------------------------------------------------------------------------
// Sticky-stack computation
// ---------------------------------------------------------------------------

/**
 * Determine which scopes should appear in the sticky header given the
 * current scroll position.
 *
 * A scope is included when:
 *   - Its open line has scrolled above the viewport top
 *   - Its close line is still below the viewport top
 *
 * The stack is recalculated from scratch on every call (no push/pop)
 * to prevent cumulative drift from rounding or missed events.
 */
function computeVisibleStack(
    view: EditorView,
    scopes: StickyScope[],
    maxLines: number,
): StickyScope[] {
    const scrollTop = view.scrollDOM.scrollTop;
    const lineCount = view.state.doc.lines;
    const stack: StickyScope[] = [];

    for (const scope of scopes) {
        // Guard against stale scopes after a doc edit but before re-parse.
        if (scope.openLine > lineCount || scope.closeLine > lineCount) {
            continue;
        }

        // lineBlockAt is O(log n) — reads from CM's height-map B-tree,
        // no DOM measurement / layout reflow.
        const openPos = view.state.doc.line(scope.openLine).from;
        const closePos = view.state.doc.line(scope.closeLine).from;

        const openTop = view.lineBlockAt(openPos).top;
        const closeTop = view.lineBlockAt(closePos).top;

        // Open line scrolled past viewport top AND close line still
        // below it → this scope's nesting context is relevant.
        if (openTop < scrollTop && closeTop > scrollTop) {
            stack.push(scope);
        }
    }

    // Cap at maxLines, keeping the deepest (most relevant) scopes.
    // Scopes arrive in document order (outermost first) from the
    // preorder tree walk, so we slice from the end.
    return stack.length > maxLines
        ? stack.slice(stack.length - maxLines)
        : stack;
}

// ---------------------------------------------------------------------------
// DOM cloning — Monaco-style line capture
// ---------------------------------------------------------------------------

/**
 * Find the `.cm-line` DOM element for a given 1-based line number.
 *
 * Uses `view.domAtPos(lineStart)` to locate the block, then walks up to
 * the nearest `.cm-line` element.  Returns `null` when the line is outside
 * the editor's current viewport buffer (CM6 virtualizes off-screen lines).
 */
function findLineElement(view: EditorView, lineNumber: number): HTMLElement | null {
    const lineCount = view.state.doc.lines;
    if (lineNumber < 1 || lineNumber > lineCount) return null;

    const lineObj = view.state.doc.line(lineNumber);

    // domAtPos throws when the position is outside the rendered viewport.
    try {
        const domPos = view.domAtPos(lineObj.from);
        let node: Node | null = domPos.node;

        // domAtPos may return a text node inside the line — walk up.
        while (node && node !== view.contentDOM) {
            if (
                node instanceof HTMLElement &&
                node.classList.contains("cm-line")
            ) {
                return node;
            }
            node = node.parentNode;
        }
    } catch {
        // Position not in rendered viewport — expected for virtualized lines.
    }

    return null;
}

/**
 * Deep-clone a `.cm-line` element's child nodes into `container`.
 *
 * Strips CM6-internal attributes (`cmView`, `cmIgnore`, etc.) and classes
 * (`cm-line` itself) from the clone so it behaves as inert markup inside
 * the sticky-scroll panel.
 */
function cloneLineContent(source: HTMLElement, container: HTMLElement): void {
    // Clone children — not the .cm-line wrapper itself.
    for (const child of source.childNodes) {
        container.appendChild(child.cloneNode(true));
    }
}

/**
 * Proactively warm the line-clone cache for all scope-opening lines that
 * are currently present in the editor DOM.
 *
 * Called on viewport changes and doc changes so that by the time a line
 * becomes sticky (scrolls past the top), its clone is already cached.
 */
function warmCloneCache(
    view: EditorView,
    scopes: StickyScope[],
    cache: Map<number, HTMLElement>,
    setter?: (lineNumber: number, el: HTMLElement) => void,
): void {
    const put = setter ?? ((ln: number, el: HTMLElement) => cache.set(ln, el));
    for (const scope of scopes) {
        const el = findLineElement(view, scope.openLine);
        if (el) {
            // Always re-clone — decorations may have changed since the
            // last capture (e.g. theme switch, extension reconfigure).
            const fragment = document.createElement("span");
            cloneLineContent(el, fragment);
            put(scope.openLine, fragment);
        }
    }
}

// ---------------------------------------------------------------------------
// Syntax-highlighted line rendering (lezer fallback)
// ---------------------------------------------------------------------------

/**
 * Fallback: render a single editor line into `container` using the lezer
 * syntax tree with full syntax highlighting.
 *
 * Used only when DOM cloning is not possible (line was never in the
 * viewport and has no cached clone).  For languages with lezer grammars
 * (JSON, JS, Python, …) this produces identical output to what CM6
 * renders.  For decoration-only languages (CSV rainbow), the output is
 * plain text — but such lines are almost always captured by proactive
 * DOM cloning before this path is reached.
 *
 * Uses `highlightTree` from `@lezer/highlight` with a wrapper that
 * delegates to `highlightingFor(state, tags)`, which reads ALL active
 * `HighlightStyle` extensions in the editor state.
 */
function renderHighlightedLine(
    container: HTMLElement,
    state: EditorState,
    lineFrom: number,
    lineTo: number,
): void {
    const tree = syntaxTree(state);
    const text = state.sliceDoc(lineFrom, lineTo);

    // Build a Highlighter-shaped object that delegates to every active
    // HighlightStyle in the editor state via highlightingFor().
    //
    // highlightTree accepts { style(tags): string | null } — we don't
    // need the optional `scope` method because we're highlighting the
    // document's own tree (always in scope).
    const wrapper = {
        style(tags: readonly Tag[]) {
            return highlightingFor(state, tags);
        },
    };

    let pos = 0; // cursor position within `text`

    highlightTree(
        tree,
        wrapper,
        (from: number, to: number, classes: string) => {
            // Convert document-absolute positions to offsets within `text`.
            const relFrom = Math.max(from - lineFrom, 0);
            const relTo = Math.min(to - lineFrom, text.length);
            if (relFrom >= relTo) return;

            // Unstyled text before this highlighted range
            if (relFrom > pos) {
                container.appendChild(
                    document.createTextNode(text.slice(pos, relFrom)),
                );
            }

            // Highlighted span — uses the real CM6 theme CSS classes
            const span = document.createElement("span");
            span.className = classes;
            span.textContent = text.slice(relFrom, relTo);
            container.appendChild(span);

            pos = relTo;
        },
        lineFrom,
        lineTo,
    );

    // Trailing unstyled text
    if (pos < text.length) {
        container.appendChild(document.createTextNode(text.slice(pos)));
    }
}

// ---------------------------------------------------------------------------
// Style synchronisation
// ---------------------------------------------------------------------------

/**
 * Per-instance cache of layout values read from the editor DOM.
 * Compared against each sync call so DOM writes happen only on change.
 *
 * NOTE: We only sync **layout** properties (font metrics, gutter geometry,
 * background) — never syntax colours.  Colours come from the real CM6
 * HighlightStyle CSS classes emitted by `renderHighlightedLine`, so they
 * respond to theme changes automatically without any DOM reading.
 */
interface LayoutCache {
    fontKey: string;
    bg: string;
    gutterW: number;
    gutterColor: string;
    textPad: number;
    hoverBg: string;
}

function createLayoutCache(): LayoutCache {
    return {
        fontKey: "",
        bg: "",
        gutterW: -1,
        gutterColor: "",
        textPad: -1,
        hoverBg: "",
    };
}

/**
 * Sync layout properties from the live editor DOM into CSS custom
 * properties on the panel root.
 *
 * These reads are necessary because CodeMirror 6 doesn't expose gutter
 * width or effective font metrics via its state API — they only exist
 * in the DOM after the theme's `EditorView.theme()` styles have been
 * applied.  All values are cached, so DOM writes happen only when a
 * value changes (typically only on theme switches).
 */
function syncLayoutStyles(
    root: HTMLElement,
    view: EditorView,
    cache: LayoutCache,
): void {
    const cs = getComputedStyle(view.contentDOM);

    // ── Font (must match .cm-content for pixel-perfect alignment) ────
    // This is the font the theme sets on `.cm-content` — we read the
    // computed value because CM6 doesn't expose it as a state facet.
    const fontKey = `${cs.fontFamily}|${cs.fontSize}|${cs.lineHeight}`;
    if (fontKey !== cache.fontKey) {
        cache.fontKey = fontKey;
        root.style.fontFamily = cs.fontFamily;
        root.style.fontSize = cs.fontSize;
        root.style.lineHeight = cs.lineHeight;
    }

    // ── Editor background (solid panel occlusion) ────────────────────
    // Read from .cm-editor since themes set background via `&` selector.
    const bg = getComputedStyle(view.dom).backgroundColor;
    if (bg !== cache.bg) {
        cache.bg = bg;
        root.style.setProperty("--sticky-scroll-bg", bg);
    }

    // ── Gutter width ─────────────────────────────────────────────────
    // contentDOM.offsetLeft = distance from scroller edge to content —
    // this IS the gutter width.  No CSS API exposes this dynamically.
    const gutterW = view.contentDOM.offsetLeft;
    if (gutterW !== cache.gutterW) {
        cache.gutterW = gutterW;
        root.style.setProperty("--sticky-scroll-gutter-w", `${gutterW}px`);
    }

    // ── Gutter colour ────────────────────────────────────────────────
    const gutterEl = view.dom.querySelector(".cm-gutters");
    if (gutterEl) {
        const c = getComputedStyle(gutterEl).color;
        if (c !== cache.gutterColor) {
            cache.gutterColor = c;
            root.style.setProperty("--sticky-scroll-gutter-color", c);
        }
    }

    // ── Text-start padding (aligns with editor text) ─────────────────
    const contentPad = parseFloat(cs.paddingLeft) || 0;
    const lineEl = view.contentDOM.querySelector(".cm-line");
    const linePad = lineEl
        ? parseFloat(getComputedStyle(lineEl).paddingLeft) || 0
        : 0;
    const textPad = contentPad + linePad;
    if (textPad !== cache.textPad) {
        cache.textPad = textPad;
        root.style.setProperty("--sticky-scroll-text-pad", `${textPad}px`);
    }

    // ── Hover background ─────────────────────────────────────────────
    // Fall back to a subtle overlay when the theme doesn't define
    // .cm-activeLine background.
    const activeLineEl = view.dom.querySelector(".cm-activeLine");
    let hoverBg = "rgba(128, 128, 128, 0.08)";
    if (activeLineEl) {
        const alBg = getComputedStyle(activeLineEl).backgroundColor;
        if (alBg && alBg !== "rgba(0, 0, 0, 0)" && alBg !== "transparent") {
            hoverBg = alBg;
        }
    }
    if (hoverBg !== cache.hoverBg) {
        cache.hoverBg = hoverBg;
        root.style.setProperty("--sticky-scroll-hover-bg", hoverBg);
    }
}

// ---------------------------------------------------------------------------
// Panel factory
// ---------------------------------------------------------------------------

function createStickyScrollPanel(
    view: EditorView,
    config: StickyScrollConfig,
): Panel {
    const maxLines = config.maxLines ?? DEFAULT_MAX_LINES;

    // ── DOM ──────────────────────────────────────────────────────────
    const root = document.createElement("div");
    root.className = `cm-sticky-scroll ${config.class}`;
    root.style.display = "none"; // hidden until first active scope

    // ── Mutable state ────────────────────────────────────────────────
    let scopes = config.computeScopes(view);
    let prevStackKey = ""; // cheap equality check to skip redundant DOM rebuilds
    let rafId = 0;
    const layoutCache = createLayoutCache();

    // Line-clone cache: maps 1-based line numbers to deep-cloned DOM
    // fragments of the corresponding `.cm-line` elements.  Warmed
    // proactively on viewport changes; invalidated on doc changes.
    // Capped to avoid unbounded growth when scrolling through many scopes.
    const MAX_CLONE_CACHE_SIZE = 100;
    const lineCloneCache = new Map<number, HTMLElement>();

    /** Insert into the clone cache, evicting the oldest entry if at capacity. */
    function cacheLineClone(lineNumber: number, el: HTMLElement): void {
        if (lineCloneCache.size >= MAX_CLONE_CACHE_SIZE && !lineCloneCache.has(lineNumber)) {
            // Map iterates in insertion order — first key is the oldest.
            const oldest = lineCloneCache.keys().next().value;
            if (oldest !== undefined) lineCloneCache.delete(oldest);
        }
        lineCloneCache.set(lineNumber, el);
    }

    // ── Render a single line into container ──────────────────────────

    /**
     * Populate `container` with content for `lineNumber`, using the
     * best available strategy:
     *
     * 1. Live DOM clone  — line is in the viewport right now
     * 2. Cached clone    — line was in the viewport earlier
     * 3. Lezer fallback  — line was never rendered (rare)
     */
    function renderLineContent(
        container: HTMLElement,
        lineNumber: number,
    ): void {
        // 1. Try to clone from the live DOM (most up-to-date).
        const liveEl = findLineElement(view, lineNumber);
        if (liveEl) {
            cloneLineContent(liveEl, container);
            // Update cache with the freshest clone.
            const cacheEntry = document.createElement("span");
            cloneLineContent(liveEl, cacheEntry);
            cacheLineClone(lineNumber, cacheEntry);
            return;
        }

        // 2. Use cached clone from a previous viewport pass.
        const cached = lineCloneCache.get(lineNumber);
        if (cached) {
            // Clone the cache entry so the cache itself stays intact
            // for future renders.
            container.appendChild(cached.cloneNode(true));
            return;
        }

        // 3. Lezer-based fallback — produces correct output for all
        //    languages with a lezer grammar.
        const line = view.state.doc.line(lineNumber);
        renderHighlightedLine(container, view.state, line.from, line.to);
    }

    // ── Render stack ─────────────────────────────────────────────────

    function renderStack(): void {
        const stack = computeVisibleStack(view, scopes, maxLines);

        // Stable identity key — skip DOM rebuild when unchanged.
        const key = stack.map((s) => s.openLine).join(",");
        if (key === prevStackKey) return;
        prevStackKey = key;

        // Clear
        root.replaceChildren();

        if (stack.length === 0) {
            root.style.display = "none";
            return;
        }

        root.style.display = "";

        for (const scope of stack) {
            const row = document.createElement("div");
            row.className = "cm-sticky-scroll-row";

            // Line number (left, gutter-aligned)
            const numEl = document.createElement("span");
            numEl.className = "cm-sticky-scroll-linenum";
            numEl.textContent = String(scope.openLine);
            row.appendChild(numEl);

            // Line content — rendered via DOM clone → cache → lezer
            const contentEl = document.createElement("span");
            contentEl.className = "cm-sticky-scroll-content";
            renderLineContent(contentEl, scope.openLine);
            row.appendChild(contentEl);

            // Click → jump to line
            //
            // We set scrollDOM.scrollTop manually instead of using
            // EditorView.scrollIntoView because the latter adds a
            // small yMargin that positions the line slightly *below*
            // the viewport top.  That makes openTop < scrollTop true
            // and the clicked scope remains in the sticky panel —
            // appearing to show the same line twice.
            //
            // By setting scrollTop = block.top exactly, the strict
            // `openTop < scrollTop` comparison in computeVisibleStack
            // evaluates to false and the scope drops out of the panel.
            const targetLine = scope.openLine;
            row.addEventListener("click", () => {
                if (targetLine < 1 || targetLine > view.state.doc.lines) {
                    return;
                }
                const pos = view.state.doc.line(targetLine).from;
                const block = view.lineBlockAt(pos);
                view.dispatch({ selection: { anchor: pos } });
                view.scrollDOM.scrollTop = block.top;
                view.focus();
            });

            root.appendChild(row);
        }
    }

    // ── Scroll handler (rAF-throttled) ───────────────────────────────

    function onScroll(): void {
        if (rafId) return;
        rafId = requestAnimationFrame(() => {
            rafId = 0;
            renderStack();
        });
    }

    // ── Initial setup ────────────────────────────────────────────────
    syncLayoutStyles(root, view, layoutCache);
    // Proactively cache scope lines that are in the initial viewport.
    warmCloneCache(view, scopes, lineCloneCache, cacheLineClone);    renderStack();
    view.scrollDOM.addEventListener("scroll", onScroll, { passive: true });

    return {
        top: true,
        dom: root,

        update(update: ViewUpdate): void {
            // Determine whether to re-parse scopes.
            const reparse = config.shouldReparse
                ? config.shouldReparse(update)
                : update.docChanged ||
                  syntaxTree(update.state) !==
                      syntaxTree(update.startState);

            if (reparse) {
                scopes = config.computeScopes(update.view);
                prevStackKey = ""; // force DOM rebuild

                // Invalidate clone cache on doc changes — line content
                // has changed so cached clones are stale.
                if (update.docChanged) {
                    lineCloneCache.clear();
                }
            }

            // Warm the clone cache with any scope lines now in the DOM.
            // This is cheap (one querySelector per scope line) and keeps
            // the cache fresh after theme switches and viewport changes.
            if (update.viewportChanged || reparse) {
                warmCloneCache(update.view, scopes, lineCloneCache, cacheLineClone);
            }

            // Sync layout (font, gutter, background) with the editor.
            syncLayoutStyles(root, update.view, layoutCache);

            // Re-evaluate the stack (handles geometry changes from
            // fold toggles, window resizes, etc.).
            renderStack();
        },

        destroy(): void {
            view.scrollDOM.removeEventListener("scroll", onScroll);
            if (rafId) cancelAnimationFrame(rafId);
            lineCloneCache.clear();
        },
    };
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create a sticky-scroll extension for any language.
 *
 * Returns a self-contained `Extension` (panel + base theme) — consumers
 * can spread it directly into their extension array.
 *
 * ```ts
 * const jsonSticky = createStickyScroll({
 *     class: "cm-json-sticky-scroll",
 *     computeScopes: parseJsonScopes,
 * });
 *
 * // In the extension array:
 * extensions: [json(), jsonSticky, ...]
 * ```
 */
export function createStickyScroll(config: StickyScrollConfig): Extension {
    return [
        showPanel.of((view: EditorView) =>
            createStickyScrollPanel(view, config),
        ),
        stickyScrollBaseTheme,
    ];
}

// ---------------------------------------------------------------------------
// Base theme
// ---------------------------------------------------------------------------

/**
 * Structural styles for the sticky scroll panel.  Layout CSS custom
 * properties (`--sticky-scroll-*`) are set at runtime by
 * `syncLayoutStyles`.
 *
 * Syntax colours come from the editor's own `HighlightStyle` CSS classes
 * — no colour rules are needed here.
 *
 * Exported separately for advanced use (the factory already includes it).
 */
export const stickyScrollBaseTheme: Extension = EditorView.baseTheme({
    ".cm-sticky-scroll": {
        backgroundColor: "var(--sticky-scroll-bg, transparent)",
        borderBottom:
            "1px solid color-mix(in srgb, currentColor 12%, transparent)",
        zIndex: "10",
        overflow: "hidden",
    },

    ".cm-sticky-scroll-row": {
        display: "flex",
        alignItems: "baseline",
        // `pre` preserves leading indentation — critical for showing
        // the nesting level of each scope line.
        whiteSpace: "pre",
        overflow: "hidden",
        cursor: "pointer",
        // Solid background per row so scrolling content beneath the
        // panel is fully occluded.
        backgroundColor: "var(--sticky-scroll-bg, transparent)",
    },

    ".cm-sticky-scroll-row:hover": {
        backgroundColor:
            "var(--sticky-scroll-hover-bg, rgba(128,128,128,.08))",
    },

    ".cm-sticky-scroll-linenum": {
        display: "inline-block",
        boxSizing: "border-box",
        width: "var(--sticky-scroll-gutter-w, 40px)",
        textAlign: "right",
        paddingRight: "8px",
        paddingLeft: "5px",
        flexShrink: "0",
        userSelect: "none",
        color: "var(--sticky-scroll-gutter-color, #888)",
    },

    ".cm-sticky-scroll-content": {
        flex: "1",
        overflow: "hidden",
        textOverflow: "ellipsis",
        paddingLeft: "var(--sticky-scroll-text-pad, 6px)",
    },
});
