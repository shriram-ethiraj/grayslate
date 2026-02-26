import type { EditorView } from 'codemirror';

/**
 * A normalized scroll anchor that maps a fraction of the editor's scroll range
 * to a fraction of the preview's scroll range.
 *
 * This ensures top (0.0) and bottom (1.0) always match between the two panes,
 * while the anchor map controls the interpolation curve in between.
 */
export interface ScrollAnchor {
    editorFraction: number;  // 0.0 to 1.0
    previewFraction: number; // 0.0 to 1.0
}

/**
 * Builds a normalized anchor map from [data-line] elements in the preview.
 */
export function buildAnchorMap(
    editorView: EditorView,
    previewEl: HTMLElement,
): ScrollAnchor[] {
    const elements = previewEl.querySelectorAll<HTMLElement>('[data-line]');
    const anchors: ScrollAnchor[] = [];

    const maxEditorScroll = editorView.scrollDOM.scrollHeight - editorView.scrollDOM.clientHeight;
    const maxPreviewScroll = previewEl.scrollHeight - previewEl.clientHeight;

    if (maxEditorScroll <= 0 || maxPreviewScroll <= 0) return [];

    const totalLines = editorView.state.doc.lines;

    for (const el of elements) {
        const lineNum = parseInt(el.getAttribute('data-line')!, 10);
        if (isNaN(lineNum) || lineNum < 1 || lineNum > totalLines) continue;

        try {
            const lineBlock = editorView.lineBlockAt(
                editorView.state.doc.line(lineNum).from
            );
            const editorFraction = Math.min(1, Math.max(0, lineBlock.top / maxEditorScroll));
            const previewFraction = Math.min(1, Math.max(0, el.offsetTop / maxPreviewScroll));

            anchors.push({ editorFraction, previewFraction });
        } catch {
            // Line may not exist during editing; skip
        }
    }

    anchors.sort((a, b) => a.editorFraction - b.editorFraction);

    const filtered: ScrollAnchor[] = [];
    for (const anchor of anchors) {
        if (
            filtered.length === 0 ||
            anchor.editorFraction - filtered[filtered.length - 1].editorFraction > 0.001
        ) {
            filtered.push(anchor);
        }
    }

    return filtered;
}

/**
 * Interpolate: editor fraction → preview fraction using anchors.
 */
function editorToPreviewFraction(anchors: ScrollAnchor[], editorFrac: number): number {
    if (anchors.length === 0) return editorFrac;

    if (editorFrac <= anchors[0].editorFraction) {
        if (anchors[0].editorFraction <= 0) return anchors[0].previewFraction;
        const t = editorFrac / anchors[0].editorFraction;
        return t * anchors[0].previewFraction;
    }

    const last = anchors[anchors.length - 1];
    if (editorFrac >= last.editorFraction) {
        const range = 1 - last.editorFraction;
        if (range <= 0) return last.previewFraction;
        const t = (editorFrac - last.editorFraction) / range;
        return last.previewFraction + t * (1 - last.previewFraction);
    }

    let lo = 0;
    let hi = anchors.length - 1;
    while (lo < hi - 1) {
        const mid = (lo + hi) >> 1;
        if (anchors[mid].editorFraction <= editorFrac) lo = mid;
        else hi = mid;
    }

    const a = anchors[lo];
    const b = anchors[hi];
    const t = (editorFrac - a.editorFraction) / (b.editorFraction - a.editorFraction || 1);
    return a.previewFraction + t * (b.previewFraction - a.previewFraction);
}

/**
 * Interpolate: preview fraction → editor fraction using anchors.
 */
function previewToEditorFraction(anchors: ScrollAnchor[], previewFrac: number): number {
    if (anchors.length === 0) return previewFrac;

    if (previewFrac <= anchors[0].previewFraction) {
        if (anchors[0].previewFraction <= 0) return anchors[0].editorFraction;
        const t = previewFrac / anchors[0].previewFraction;
        return t * anchors[0].editorFraction;
    }

    const last = anchors[anchors.length - 1];
    if (previewFrac >= last.previewFraction) {
        const range = 1 - last.previewFraction;
        if (range <= 0) return last.editorFraction;
        const t = (previewFrac - last.previewFraction) / range;
        return last.editorFraction + t * (1 - last.editorFraction);
    }

    let lo = 0;
    let hi = anchors.length - 1;
    while (lo < hi - 1) {
        const mid = (lo + hi) >> 1;
        if (anchors[mid].previewFraction <= previewFrac) lo = mid;
        else hi = mid;
    }

    const a = anchors[lo];
    const b = anchors[hi];
    const t = (previewFrac - a.previewFraction) / (b.previewFraction - a.previewFraction || 1);
    return a.editorFraction + t * (b.editorFraction - a.editorFraction);
}

/**
 * Creates a bidirectional scroll sync controller.
 *
 * Uses mouse/pointer tracking to determine which pane the user is interacting
 * with, and only syncs FROM the active pane TO the passive pane. This
 * completely eliminates feedback loops — scroll events on the passive pane
 * (caused by our syncing) never try to sync back.
 *
 * The preview side uses lerp-based smooth animation for fluid scrolling.
 */
export function createScrollSync(
    editorView: EditorView,
    previewEl: HTMLElement
): () => void {
    const editorScrollEl = editorView.scrollDOM;
    let anchors: ScrollAnchor[] = [];

    // Track which pane the user's pointer is over.
    // Only the active pane drives scroll sync.
    let activePane: 'editor' | 'preview' | null = null;

    // Lerp animation state for smooth preview scrolling
    let previewTargetScroll = previewEl.scrollTop;
    let previewLerpRafId: number | null = null;
    const LERP_FACTOR = 0.25;
    const LERP_THRESHOLD = 0.5;

    function refreshAnchors() {
        anchors = buildAnchorMap(editorView, previewEl);
    }

    refreshAnchors();

    const observer = new MutationObserver(() => {
        setTimeout(refreshAnchors, 100);
    });
    observer.observe(previewEl, { childList: true, subtree: true });

    function onImageLoad() {
        refreshAnchors();
    }
    previewEl.addEventListener('load', onImageLoad, { capture: true });

    // --- Pointer tracking ---
    // We need to track the parent container of both panes for mouseenter,
    // but since we only have references to the scroll elements, we track
    // them individually.

    function onEditorPointerEnter() {
        activePane = 'editor';
    }

    function onPreviewPointerEnter() {
        activePane = 'preview';
        if (previewLerpRafId) {
            cancelAnimationFrame(previewLerpRafId);
            previewLerpRafId = null;
        }
    }

    // Use the editor's parent (which wraps the CodeMirror DOM) for pointer tracking
    const editorContainer = editorScrollEl.closest('.cm-editor') || editorScrollEl;
    editorContainer.addEventListener('pointerenter', onEditorPointerEnter);
    previewEl.addEventListener('pointerenter', onPreviewPointerEnter);

    // --- Lerp animation for smooth preview scroll ---
    function animatePreviewScroll() {
        const diff = previewTargetScroll - previewEl.scrollTop;

        if (Math.abs(diff) < LERP_THRESHOLD) {
            previewEl.scrollTop = previewTargetScroll;
            previewLerpRafId = null;
            return;
        }

        previewEl.scrollTop += diff * LERP_FACTOR;
        previewLerpRafId = requestAnimationFrame(animatePreviewScroll);
    }

    // --- Sync handlers ---
    function syncPreviewFromEditor() {
        if (activePane !== 'editor') return;

        const maxEditorScroll = editorScrollEl.scrollHeight - editorScrollEl.clientHeight;
        const maxPreviewScroll = previewEl.scrollHeight - previewEl.clientHeight;

        if (maxEditorScroll <= 0 || maxPreviewScroll <= 0) return;

        const editorFrac = editorScrollEl.scrollTop / maxEditorScroll;
        const previewFrac = editorToPreviewFraction(anchors, editorFrac);
        previewTargetScroll = previewFrac * maxPreviewScroll;

        if (!previewLerpRafId) {
            previewLerpRafId = requestAnimationFrame(animatePreviewScroll);
        }
    }

    function syncEditorFromPreview() {
        if (activePane !== 'preview') return;

        requestAnimationFrame(() => {
            const maxEditorScroll = editorScrollEl.scrollHeight - editorScrollEl.clientHeight;
            const maxPreviewScroll = previewEl.scrollHeight - previewEl.clientHeight;

            if (maxEditorScroll <= 0 || maxPreviewScroll <= 0) return;

            const previewFrac = previewEl.scrollTop / maxPreviewScroll;
            const editorFrac = previewToEditorFraction(anchors, previewFrac);
            const targetScrollTop = editorFrac * maxEditorScroll;

            if (Math.abs(editorScrollEl.scrollTop - targetScrollTop) > 1) {
                editorScrollEl.scrollTop = targetScrollTop;
            }
        });
    }

    editorScrollEl.addEventListener('scroll', syncPreviewFromEditor, { passive: true });
    previewEl.addEventListener('scroll', syncEditorFromPreview, { passive: true });

    return () => {
        editorScrollEl.removeEventListener('scroll', syncPreviewFromEditor);
        previewEl.removeEventListener('scroll', syncEditorFromPreview);
        editorContainer.removeEventListener('pointerenter', onEditorPointerEnter);
        previewEl.removeEventListener('pointerenter', onPreviewPointerEnter);
        previewEl.removeEventListener('load', onImageLoad, { capture: true });
        observer.disconnect();
        if (previewLerpRafId) cancelAnimationFrame(previewLerpRafId);
    };
}
