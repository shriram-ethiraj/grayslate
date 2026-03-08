/**
 * Custom scroll-scaled virtualizer that bypasses WebKit's max element height
 * limit (33,554,432px / 2^25). For datasets whose total pixel height exceeds
 * a safe cap, scroll position is mapped proportionally to row indices so all
 * rows remain reachable via the native scrollbar.
 */
export function useScrollVirtualizer(options: {
    count: () => number;
    getScrollElement: () => HTMLElement | undefined;
    estimateSize: () => number;
    headerHeight?: () => number;
    overscan?: number;
}) {
    const MAX_SCROLL_HEIGHT = 30_000_000;
    /**
     * Safety cap: never render more virtual items than this, regardless of
     * what containerHeight the ResizeObserver reports.  Prevents runaway
     * DOM creation when the scroll-container's CSS height is unconstrained
     * (e.g. broken flex chain after a layout change).
     */
    const MAX_VIRTUAL_ITEMS = 200;

    let scrollTop = $state(0);
    let containerHeight = $state(600);
    let effectiveScrollHeight = $state(34);

    const rowHeight = $derived(options.estimateSize());
    const headerHeight = $derived(options.headerHeight?.() ?? 34);
    const totalCount = $derived(options.count());
    const overscan = $derived(options.overscan ?? 10);

    const trueTotalRowHeight = $derived(totalCount * rowHeight);
    const needsScaling = $derived(trueTotalRowHeight > MAX_SCROLL_HEIGHT);
    const virtualTotalHeight = $derived(
        (needsScaling ? MAX_SCROLL_HEIGHT : trueTotalRowHeight) + headerHeight,
    );

    const effectiveTotalHeight = $derived(
        needsScaling
            ? Math.min(
                  virtualTotalHeight,
                  Math.max(headerHeight, effectiveScrollHeight),
              )
            : virtualTotalHeight,
    );

    function getVisibleCount(viewportHeight: number) {
        return Math.max(
            1,
            Math.ceil(Math.max(0, viewportHeight - headerHeight) / rowHeight),
        );
    }

    function getMaxRowScroll(viewportHeight: number) {
        return Math.max(0, effectiveTotalHeight - viewportHeight - headerHeight);
    }

    function updateScrollMetrics(el: HTMLElement) {
        const measuredContainerHeight = el.clientHeight;
        containerHeight = Math.min(measuredContainerHeight, window.innerHeight * 3);

        const measuredScrollHeight = el.scrollHeight;
        effectiveScrollHeight = measuredScrollHeight > 0 ? measuredScrollHeight : virtualTotalHeight;

        const maxScrollTop = Math.max(0, measuredScrollHeight - containerHeight);
        scrollTop = Math.min(el.scrollTop, maxScrollTop);
    }

    $effect(() => {
        const el = options.getScrollElement();
        if (!el) return;

        const observer = new ResizeObserver((entries) => {
            for (const entry of entries) {
                if (entry.target === el) {
                    const h = entry.contentRect.height;
                    // Sanity-check: if the reported height is larger than
                    // 3× the window, the container's CSS is unconstrained
                    // (e.g. flex chain broken).  Clamp to the window height
                    // so we never spawn millions of virtual items.
                    containerHeight = Math.min(h, window.innerHeight * 3);
                    effectiveScrollHeight = el.scrollHeight || virtualTotalHeight;
                }
            }
        });
        observer.observe(el);

        const onScroll = () => {
            updateScrollMetrics(el);
        };
        el.addEventListener("scroll", onScroll, { passive: true });

        updateScrollMetrics(el);

        return () => {
            observer.disconnect();
            el.removeEventListener("scroll", onScroll);
        };
    });

    $effect(() => {
        const el = options.getScrollElement();
        if (!el) {
            effectiveScrollHeight = virtualTotalHeight;
            return;
        }

        const frame = requestAnimationFrame(() => {
            updateScrollMetrics(el);
        });

        return () => {
            cancelAnimationFrame(frame);
        };
    });

    const virtualItems = $derived.by(() => {
        if (totalCount === 0) return [];

        const visibleCount = getVisibleCount(containerHeight);
        const rowScrollTop = Math.max(0, scrollTop - headerHeight);

        let firstVisibleRow: number;

        if (!needsScaling) {
            firstVisibleRow = Math.floor(rowScrollTop / rowHeight);
        } else {
            const maxRowScroll = Math.max(1, getMaxRowScroll(containerHeight));
            const fraction = Math.min(1, rowScrollTop / maxRowScroll);
            const maxFirstRow = Math.max(0, totalCount - visibleCount);
            firstVisibleRow = Math.floor(fraction * maxFirstRow);
        }

        const maxFirst = Math.max(0, totalCount - visibleCount);
        firstVisibleRow = Math.max(0, Math.min(firstVisibleRow, maxFirst));

        const startIdx = Math.max(0, firstVisibleRow - overscan);
        const rawEndIdx = Math.min(totalCount - 1, firstVisibleRow + visibleCount + overscan);
        // Hard-cap the window to MAX_VIRTUAL_ITEMS so a broken layout can
        // never cause us to create a DOM-exploding number of rows.
        const endIdx = Math.min(rawEndIdx, startIdx + MAX_VIRTUAL_ITEMS - 1);

        let blockStart: number;
        if (!needsScaling) {
            blockStart = headerHeight + startIdx * rowHeight;
        } else {
            blockStart = headerHeight + rowScrollTop - (firstVisibleRow - startIdx) * rowHeight;
            if (endIdx === totalCount - 1) {
                const renderedHeight = (endIdx - startIdx + 1) * rowHeight;
                blockStart = Math.max(blockStart, effectiveTotalHeight - renderedHeight);
            }
        }

        const items: { index: number; start: number; size: number }[] = [];
        for (let i = startIdx; i <= endIdx; i++) {
            items.push({
                index: i,
                start: blockStart + (i - startIdx) * rowHeight,
                size: rowHeight,
            });
        }
        return items;
    });

    return {
        get virtualItems() {
            return virtualItems;
        },
        get totalSize() {
            return virtualTotalHeight;
        },
        scrollToIndex(
            index: number,
            scrollOpts?: { align?: "start" | "center" | "end" | "auto" },
        ) {
            const el = options.getScrollElement();
            if (!el) return;

            const align = scrollOpts?.align ?? "auto";

            if (!needsScaling) {
                const itemTop = headerHeight + index * rowHeight;
                const itemBottom = itemTop + rowHeight;
                const viewTop = el.scrollTop;
                const viewBottom = viewTop + containerHeight;

                // "auto": only scroll if the item is outside the visible area
                if (align === "auto") {
                    if (itemTop >= viewTop && itemBottom <= viewBottom) return;
                    if (itemTop < viewTop) {
                        el.scrollTo({ top: Math.max(0, itemTop) });
                    } else {
                        el.scrollTo({ top: Math.max(0, itemBottom - containerHeight) });
                    }
                } else {
                    let top = itemTop;
                    if (align === "center") {
                        top -= (containerHeight - rowHeight) / 2;
                    } else if (align === "end") {
                        top -= containerHeight - rowHeight;
                    }
                    el.scrollTo({ top: Math.max(0, top) });
                }
            } else {
                const visibleCount = getVisibleCount(containerHeight);
                const maxFirstRow = Math.max(0, totalCount - visibleCount);
                const maxRowScroll = getMaxRowScroll(containerHeight);

                // "auto": check if already visible in scaled mode
                if (align === "auto") {
                    const rowScrollTop = Math.max(0, el.scrollTop - headerHeight);
                    const fraction = Math.min(1, rowScrollTop / Math.max(1, maxRowScroll));
                    const firstVisibleRow = Math.floor(fraction * maxFirstRow);
                    const lastVisibleRow = firstVisibleRow + visibleCount - 1;
                    if (index >= firstVisibleRow && index <= lastVisibleRow) return;
                }

                let targetFirstRow = index;
                if (align === "center") {
                    targetFirstRow = index - Math.floor((visibleCount - 1) / 2);
                } else if (align === "end") {
                    targetFirstRow = index - visibleCount + 1;
                }

                targetFirstRow = Math.max(0, Math.min(targetFirstRow, maxFirstRow));

                const fraction =
                    maxFirstRow === 0 ? 0 : Math.min(1, targetFirstRow / maxFirstRow);
                el.scrollTo({ top: headerHeight + fraction * maxRowScroll });
            }
        },
    };
}
