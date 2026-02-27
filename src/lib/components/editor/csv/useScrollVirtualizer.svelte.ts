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
    overscan?: number;
}) {
    const MAX_SCROLL_HEIGHT = 30_000_000;
    const HEADER_HEIGHT = 34;

    let scrollTop = $state(0);
    let containerHeight = $state(600);

    const rowHeight = $derived(options.estimateSize());
    const totalCount = $derived(options.count());
    const overscan = $derived(options.overscan ?? 10);

    const trueTotalRowHeight = $derived(totalCount * rowHeight);
    const needsScaling = $derived(trueTotalRowHeight > MAX_SCROLL_HEIGHT);
    const virtualTotalHeight = $derived(
        (needsScaling ? MAX_SCROLL_HEIGHT : trueTotalRowHeight) + HEADER_HEIGHT,
    );

    $effect(() => {
        const el = options.getScrollElement();
        if (!el) return;

        const observer = new ResizeObserver((entries) => {
            for (const entry of entries) {
                if (entry.target === el) {
                    containerHeight = entry.contentRect.height;
                }
            }
        });
        observer.observe(el);

        const onScroll = () => {
            scrollTop = el.scrollTop;
        };
        el.addEventListener("scroll", onScroll, { passive: true });

        containerHeight = el.clientHeight;
        scrollTop = el.scrollTop;

        return () => {
            observer.disconnect();
            el.removeEventListener("scroll", onScroll);
        };
    });

    const virtualItems = $derived.by(() => {
        if (totalCount === 0) return [];

        const visibleCount = Math.ceil(
            Math.max(0, containerHeight - HEADER_HEIGHT) / rowHeight,
        );
        const rowScrollTop = Math.max(0, scrollTop - HEADER_HEIGHT);

        let firstVisibleRow: number;

        if (!needsScaling) {
            firstVisibleRow = Math.floor(rowScrollTop / rowHeight);
        } else {
            const maxScroll = Math.max(1, virtualTotalHeight - containerHeight);
            const fraction = Math.min(1, rowScrollTop / Math.max(1, maxScroll));
            const maxFirstRow = Math.max(0, totalCount - visibleCount);
            firstVisibleRow = Math.round(fraction * maxFirstRow);
        }

        const maxFirst = Math.max(0, totalCount - visibleCount);
        firstVisibleRow = Math.max(0, Math.min(firstVisibleRow, maxFirst));

        const startIdx = Math.max(0, firstVisibleRow - overscan);
        const endIdx = Math.min(totalCount - 1, firstVisibleRow + visibleCount + overscan);

        let blockStart: number;
        if (!needsScaling) {
            blockStart = HEADER_HEIGHT + startIdx * rowHeight;
        } else {
            blockStart = scrollTop - (firstVisibleRow - startIdx) * rowHeight;
            if (endIdx === totalCount - 1) {
                const renderedHeight = (endIdx - startIdx + 1) * rowHeight;
                blockStart = Math.max(blockStart, virtualTotalHeight - renderedHeight);
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

            if (!needsScaling) {
                let top = HEADER_HEIGHT + index * rowHeight;
                if (scrollOpts?.align === "center") {
                    top -= (containerHeight - rowHeight) / 2;
                } else if (scrollOpts?.align === "end") {
                    top -= containerHeight - rowHeight;
                }
                el.scrollTo({ top: Math.max(0, top) });
            } else {
                const visibleCount = Math.ceil(
                    Math.max(0, containerHeight - HEADER_HEIGHT) / rowHeight,
                );
                const maxFirstRow = Math.max(1, totalCount - visibleCount);
                const fraction = Math.min(1, index / maxFirstRow);
                const maxScroll = Math.max(0, virtualTotalHeight - containerHeight);
                el.scrollTo({ top: fraction * maxScroll });
            }
        },
    };
}
