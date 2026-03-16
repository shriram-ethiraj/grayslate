import type { EditorView } from 'codemirror';

/**
 * A normalized anchor that maps logical markdown source progress to preview
 * scroll progress.
 *
 * linePercent is the canonical position in the source document. Both the
 * editor and preview translate through this shared space instead of mapping
 * pixels directly to each other.
 */
export interface ScrollAnchor {
    linePercent: number;
    previewFraction: number;
}

/**
 * Builds a normalized line-percent anchor map from [data-line] elements in the preview.
 */
export function buildAnchorMap(
    editorView: EditorView,
    previewEl: HTMLElement,
): ScrollAnchor[] {
    const elements = previewEl.querySelectorAll<HTMLElement>('[data-line]');
    const anchors: ScrollAnchor[] = [];
    const maxPreviewScroll = previewEl.scrollHeight - previewEl.clientHeight;

    if (maxPreviewScroll <= 0) return [];

    const totalLines = editorView.state.doc.lines;
    const totalLineSpan = Math.max(totalLines, 1);

    anchors.push({ linePercent: 0, previewFraction: 0 });

    for (const el of elements) {
        const lineNum = parseInt(el.getAttribute('data-line')!, 10);
        if (isNaN(lineNum) || lineNum < 1 || lineNum > totalLines) continue;

        const linePercent = Math.min(1, Math.max(0, (lineNum - 1) / totalLineSpan));
        const previewFraction = Math.min(1, Math.max(0, el.offsetTop / maxPreviewScroll));

        anchors.push({ linePercent, previewFraction });
    }

    anchors.push({ linePercent: 1, previewFraction: 1 });

    anchors.sort((a, b) => a.linePercent - b.linePercent);

    const filtered: ScrollAnchor[] = [];
    for (const anchor of anchors) {
        if (
            filtered.length === 0 ||
            anchor.linePercent - filtered[filtered.length - 1].linePercent > 0.001
        ) {
            filtered.push(anchor);
        } else if (anchor.previewFraction > filtered[filtered.length - 1].previewFraction) {
            filtered[filtered.length - 1] = anchor;
        }
    }

    return filtered;
}

/**
 * Interpolate: source line percent → preview fraction using anchors.
 */
function linePercentToPreviewFraction(anchors: ScrollAnchor[], linePercent: number): number {
    if (anchors.length === 0) return linePercent;

    if (linePercent <= anchors[0].linePercent) {
        if (anchors[0].linePercent <= 0) return anchors[0].previewFraction;
        const t = linePercent / anchors[0].linePercent;
        return t * anchors[0].previewFraction;
    }

    const last = anchors[anchors.length - 1];
    if (linePercent >= last.linePercent) {
        const range = 1 - last.linePercent;
        if (range <= 0) return last.previewFraction;
        const t = (linePercent - last.linePercent) / range;
        return last.previewFraction + t * (1 - last.previewFraction);
    }

    let lo = 0;
    let hi = anchors.length - 1;
    while (lo < hi - 1) {
        const mid = (lo + hi) >> 1;
        if (anchors[mid].linePercent <= linePercent) lo = mid;
        else hi = mid;
    }

    const a = anchors[lo];
    const b = anchors[hi];
    const t = (linePercent - a.linePercent) / (b.linePercent - a.linePercent || 1);
    return a.previewFraction + t * (b.previewFraction - a.previewFraction);
}

/**
 * Interpolate: preview fraction → source line percent using anchors.
 */
function previewToLinePercent(anchors: ScrollAnchor[], previewFrac: number): number {
    if (anchors.length === 0) return previewFrac;

    if (previewFrac <= anchors[0].previewFraction) {
        if (anchors[0].previewFraction <= 0) return anchors[0].linePercent;
        const t = previewFrac / anchors[0].previewFraction;
        return t * anchors[0].linePercent;
    }

    const last = anchors[anchors.length - 1];
    if (previewFrac >= last.previewFraction) {
        const range = 1 - last.previewFraction;
        if (range <= 0) return last.linePercent;
        const t = (previewFrac - last.previewFraction) / range;
        return last.linePercent + t * (1 - last.linePercent);
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
    return a.linePercent + t * (b.linePercent - a.linePercent);
}

function clampFraction(value: number): number {
    return Math.min(1, Math.max(0, value));
}

function getEditorLinePercent(editorView: EditorView): number {
    const totalLines = editorView.state.doc.lines;
    if (totalLines <= 1) return 0;

    const scrollTop = editorView.scrollDOM.scrollTop;
    const currentBlock = editorView.lineBlockAtHeight(scrollTop);
    const currentLine = editorView.state.doc.lineAt(currentBlock.from);
    const currentLineBlock = editorView.lineBlockAt(currentLine.from);

    let nextLineTop = editorView.scrollDOM.scrollHeight - editorView.scrollDOM.clientHeight;
    if (currentLine.number < totalLines) {
        nextLineTop = editorView.lineBlockAt(
            editorView.state.doc.line(currentLine.number + 1).from
        ).top;
    }

    const blockRange = Math.max(1, nextLineTop - currentLineBlock.top);
    const withinLine = clampFraction((scrollTop - currentLineBlock.top) / blockRange);
    return clampFraction(((currentLine.number - 1) + withinLine) / totalLines);
}

function getEditorScrollTopForLinePercent(editorView: EditorView, linePercent: number): number {
    const totalLines = editorView.state.doc.lines;
    const maxEditorScroll = editorView.scrollDOM.scrollHeight - editorView.scrollDOM.clientHeight;
    if (totalLines <= 1 || maxEditorScroll <= 0) return 0;

    const sourceProgress = clampFraction(linePercent) * totalLines;
    const sourceLineIndex = Math.min(totalLines - 1, Math.floor(sourceProgress));
    const intraLine = clampFraction(sourceProgress - sourceLineIndex);
    const lineNumber = sourceLineIndex + 1;
    const lineBlock = editorView.lineBlockAt(editorView.state.doc.line(lineNumber).from);

    let nextLineTop = maxEditorScroll;
    if (lineNumber < totalLines) {
        nextLineTop = editorView.lineBlockAt(
            editorView.state.doc.line(lineNumber + 1).from
        ).top;
    }

    return Math.min(
        maxEditorScroll,
        Math.max(0, lineBlock.top + (nextLineTop - lineBlock.top) * intraLine),
    );
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
    let destroyed = false;

    // Lerp constants — both panes use the same easing
    const LERP_FACTOR = 0.25;
    const LERP_THRESHOLD = 0.5;
    const MUTATION_REFRESH_DELAY = 90;
    const RESIZE_REFRESH_DELAY = 60;
    const IMAGE_REFRESH_DELAY = 140;
    const IMAGE_SETTLE_DELAY = 40;

    // Preview lerp state
    let previewTargetScroll = previewEl.scrollTop;
    let previewLerpRafId: number | null = null;

    // Editor lerp state
    let editorTargetScroll = editorScrollEl.scrollTop;
    let editorLerpRafId: number | null = null;

    // Deferred anchor refresh state
    let refreshTimerId: ReturnType<typeof setTimeout> | null = null;
    let refreshRafId: number | null = null;
    let pendingImages = new Set<HTMLImageElement>();
    let refreshNeedsAnchorRebuild = false;

    function startPreviewAnimation() {
        if (previewLerpRafId == null) {
            previewLerpRafId = requestAnimationFrame(animatePreviewScroll);
        }
    }

    function startEditorAnimation() {
        if (editorLerpRafId == null) {
            editorLerpRafId = requestAnimationFrame(animateEditorScroll);
        }
    }

    function collectPendingImages() {
        pendingImages = new Set(
            Array.from(previewEl.querySelectorAll('img')).filter((image) => !image.complete)
        );
    }

    function applyAnchorRefresh(rebuildAnchors: boolean) {
        const maxPreviewScrollBefore = previewEl.scrollHeight - previewEl.clientHeight;
        const editorLinePercent =
            activePane === 'editor' ? getEditorLinePercent(editorView) : 0;
        const previewLinePercent =
            activePane === 'preview' && maxPreviewScrollBefore > 0
                ? previewToLinePercent(anchors, previewEl.scrollTop / maxPreviewScrollBefore)
                : 0;

        if (rebuildAnchors) {
            anchors = buildAnchorMap(editorView, previewEl);
        }

        if (activePane === 'editor') {
            const maxPreviewScrollAfter = previewEl.scrollHeight - previewEl.clientHeight;
            if (maxPreviewScrollAfter <= 0) return;

            const nextPreviewFraction = linePercentToPreviewFraction(anchors, editorLinePercent);
            previewTargetScroll = nextPreviewFraction * maxPreviewScrollAfter;
            startPreviewAnimation();
            return;
        }

        if (activePane === 'preview') {
            editorTargetScroll = getEditorScrollTopForLinePercent(editorView, previewLinePercent);
            startEditorAnimation();
        }
    }

    function clearScheduledRefresh() {
        if (refreshTimerId != null) {
            clearTimeout(refreshTimerId);
            refreshTimerId = null;
        }
        if (refreshRafId != null) {
            cancelAnimationFrame(refreshRafId);
            refreshRafId = null;
        }
        refreshNeedsAnchorRebuild = false;
    }

    function scheduleAnchorRefresh(
        delay = MUTATION_REFRESH_DELAY,
        options?: { rebuildAnchors?: boolean }
    ) {
        if (destroyed) return;
        refreshNeedsAnchorRebuild =
            refreshNeedsAnchorRebuild || Boolean(options?.rebuildAnchors);

        // Cancel both the pending timer and any in-flight RAF so that a rapid
        // sequence of calls never causes a double applyAnchorRefresh().
        if (refreshTimerId != null) clearTimeout(refreshTimerId);
        if (refreshRafId != null) {
            cancelAnimationFrame(refreshRafId);
            refreshRafId = null;
        }

        refreshTimerId = setTimeout(() => {
            refreshTimerId = null;
            refreshRafId = requestAnimationFrame(() => {
                const rebuildAnchors = refreshNeedsAnchorRebuild;
                refreshNeedsAnchorRebuild = false;
                refreshRafId = null;
                applyAnchorRefresh(rebuildAnchors);
            });
        }, delay);
    }

    collectPendingImages();
    anchors = buildAnchorMap(editorView, previewEl);

    const observer = new MutationObserver(() => {
        collectPendingImages();
        scheduleAnchorRefresh(MUTATION_REFRESH_DELAY, { rebuildAnchors: true });
    });
    observer.observe(previewEl, { childList: true, subtree: true });

    const resizeObserver = new ResizeObserver((entries) => {
        const previewResized = entries.some(({ target }) => target === previewEl);
        const editorResized = entries.some(({ target }) => target === editorScrollEl);
        if (!previewResized && !editorResized) return;

        scheduleAnchorRefresh(RESIZE_REFRESH_DELAY, {
            rebuildAnchors: previewResized,
        });
    });
    resizeObserver.observe(previewEl);
    resizeObserver.observe(editorScrollEl);

    function onPreviewResourceLoad(event: Event) {
        if (!(event.target instanceof HTMLImageElement)) return;

        pendingImages.delete(event.target);

        scheduleAnchorRefresh(
            pendingImages.size === 0 ? IMAGE_SETTLE_DELAY : IMAGE_REFRESH_DELAY,
            { rebuildAnchors: true }
        );
    }
    previewEl.addEventListener('load', onPreviewResourceLoad, true);
    previewEl.addEventListener('error', onPreviewResourceLoad, true);

    // --- Active pane tracking ---
    function onEditorInteraction() {
        if (activePane !== 'editor') {
            activePane = 'editor';
            if (editorLerpRafId != null) {
                cancelAnimationFrame(editorLerpRafId);
                editorLerpRafId = null;
            }
        }
    }

    function onPreviewInteraction() {
        if (activePane !== 'preview') {
            activePane = 'preview';
            if (previewLerpRafId != null) {
                cancelAnimationFrame(previewLerpRafId);
                previewLerpRafId = null;
            }
        }
    }

    const editorContainer = editorScrollEl.closest('.cm-editor') || editorScrollEl;

    // Track all meaningful ways a user can take ownership of a pane so that
    // hover-then-scroll, touch, keyboard focus, and wheel events all register correctly.
    const interactionEvents = ['pointerenter', 'pointermove', 'pointerdown', 'focusin', 'wheel', 'touchstart'] as const;

    interactionEvents.forEach(evt => {
        editorContainer.addEventListener(evt, onEditorInteraction, { passive: true });
        previewEl.addEventListener(evt, onPreviewInteraction, { passive: true });
    });

    // --- Lerp animations ---
    function animatePreviewScroll() {
        if (destroyed) { previewLerpRafId = null; return; }
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
        if (destroyed) { editorLerpRafId = null; return; }
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

        const maxPreviewScroll = previewEl.scrollHeight - previewEl.clientHeight;
        if (maxPreviewScroll <= 0) return;

        const editorLinePercent = getEditorLinePercent(editorView);
        const previewFrac = linePercentToPreviewFraction(anchors, editorLinePercent);
        previewTargetScroll = previewFrac * maxPreviewScroll;
        startPreviewAnimation();
    }

    function syncEditorFromPreview() {
        if (!activePane) activePane = 'preview';
        if (activePane !== 'preview') return;

        const maxPreviewScroll = previewEl.scrollHeight - previewEl.clientHeight;
        if (maxPreviewScroll <= 0) return;

        const previewFrac = previewEl.scrollTop / maxPreviewScroll;
        const linePercent = previewToLinePercent(anchors, previewFrac);
        editorTargetScroll = getEditorScrollTopForLinePercent(editorView, linePercent);
        startEditorAnimation();
    }

    editorScrollEl.addEventListener('scroll', syncPreviewFromEditor, { passive: true });
    previewEl.addEventListener('scroll', syncEditorFromPreview, { passive: true });

    return () => {
        destroyed = true;
        clearScheduledRefresh();
        editorScrollEl.removeEventListener('scroll', syncPreviewFromEditor);
        previewEl.removeEventListener('scroll', syncEditorFromPreview);
        interactionEvents.forEach(evt => {
            editorContainer.removeEventListener(evt, onEditorInteraction);
            previewEl.removeEventListener(evt, onPreviewInteraction);
        });
        previewEl.removeEventListener('load', onPreviewResourceLoad, true);
        previewEl.removeEventListener('error', onPreviewResourceLoad, true);
        observer.disconnect();
        resizeObserver.disconnect();
        pendingImages.clear();
        if (previewLerpRafId != null) cancelAnimationFrame(previewLerpRafId);
        if (editorLerpRafId != null) cancelAnimationFrame(editorLerpRafId);
    };
}
