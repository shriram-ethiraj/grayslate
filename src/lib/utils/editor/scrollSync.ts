import type { EditorView } from 'codemirror';

/**
 * A normalized scroll anchor that maps a fraction of the editor's scroll range
 * to a fraction of the preview's scroll range.
 *
 * This ensures top (0.0) and bottom (1.0) always match between the two panes,
 * while the anchor map controls the interpolation curve in between.
 */
export interface ScrollAnchor {
    editorFraction: number;
    previewFraction: number;
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
 * Uses pointer tracking to determine which pane the user is interacting with,
 * and only syncs FROM the active pane TO the passive pane.
 *
 * Both directions use lerp-based smooth animation for fluid scrolling.
 */
export function createScrollSync(
    editorView: EditorView,
    previewEl: HTMLElement
): () => void {
    const editorScrollEl = editorView.scrollDOM;
    let anchors: ScrollAnchor[] = [];
    let activePane: 'editor' | 'preview' | null = null;

    // Lerp constants — both panes use the same easing
    const LERP_FACTOR = 0.25;
    const LERP_THRESHOLD = 0.5;

    // Preview lerp state
    let previewTargetScroll = previewEl.scrollTop;
    let previewLerpRafId: number | null = null;

    // Editor lerp state
    let editorTargetScroll = editorScrollEl.scrollTop;
    let editorLerpRafId: number | null = null;

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

    // --- Active pane tracking ---
    function onEditorInteraction() {
        if (activePane !== 'editor') {
            activePane = 'editor';
            if (editorLerpRafId) {
                cancelAnimationFrame(editorLerpRafId);
                editorLerpRafId = null;
            }
        }
    }

    function onPreviewInteraction() {
        if (activePane !== 'preview') {
            activePane = 'preview';
            if (previewLerpRafId) {
                cancelAnimationFrame(previewLerpRafId);
                previewLerpRafId = null;
            }
        }
    }

    const editorContainer = editorScrollEl.closest('.cm-editor') || editorScrollEl;

    // Add multiple interaction listeners to accurately track the active pane
    const interactionEvents = ['pointerenter', 'pointermove', 'pointerdown', 'focusin', 'wheel', 'touchstart'];

    interactionEvents.forEach(evt => {
        editorContainer.addEventListener(evt, onEditorInteraction, { passive: true });
        previewEl.addEventListener(evt, onPreviewInteraction, { passive: true });
    });

    // --- Lerp animations ---
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

    function animateEditorScroll() {
        const diff = editorTargetScroll - editorScrollEl.scrollTop;
        if (Math.abs(diff) < LERP_THRESHOLD) {
            editorScrollEl.scrollTop = editorTargetScroll;
            editorLerpRafId = null;
            return;
        }
        editorScrollEl.scrollTop += diff * LERP_FACTOR;
        editorLerpRafId = requestAnimationFrame(animateEditorScroll);
    }

    // --- Sync handlers ---
    function syncPreviewFromEditor() {
        if (!activePane) activePane = 'editor';
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
        if (!activePane) activePane = 'preview';
        if (activePane !== 'preview') return;

        const maxEditorScroll = editorScrollEl.scrollHeight - editorScrollEl.clientHeight;
        const maxPreviewScroll = previewEl.scrollHeight - previewEl.clientHeight;
        if (maxEditorScroll <= 0 || maxPreviewScroll <= 0) return;

        const previewFrac = previewEl.scrollTop / maxPreviewScroll;
        const editorFrac = previewToEditorFraction(anchors, previewFrac);
        editorTargetScroll = editorFrac * maxEditorScroll;

        if (!editorLerpRafId) {
            editorLerpRafId = requestAnimationFrame(animateEditorScroll);
        }
    }

    editorScrollEl.addEventListener('scroll', syncPreviewFromEditor, { passive: true });
    previewEl.addEventListener('scroll', syncEditorFromPreview, { passive: true });

    return () => {
        editorScrollEl.removeEventListener('scroll', syncPreviewFromEditor);
        previewEl.removeEventListener('scroll', syncEditorFromPreview);
        interactionEvents.forEach(evt => {
            editorContainer.removeEventListener(evt, onEditorInteraction);
            previewEl.removeEventListener(evt, onPreviewInteraction);
        });
        previewEl.removeEventListener('load', onImageLoad, { capture: true });
        observer.disconnect();
        if (previewLerpRafId) cancelAnimationFrame(previewLerpRafId);
        if (editorLerpRafId) cancelAnimationFrame(editorLerpRafId);
    };
}
