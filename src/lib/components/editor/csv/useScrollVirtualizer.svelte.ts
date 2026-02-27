/**
 * Custom scroll-scaled virtualizer that bypasses WebKit's max element height
 * limit (33,554,432px / 2^25).  For datasets whose total pixel height exceeds
 * a safe cap, scroll position is mapped proportionally to row indices so all
 * rows remain reachable via the native scrollbar.
 */
export function useScrollVirtualizer(options: {
    count: () => number;
    getScrollElement: () => HTMLElement | undefined;
    estimateSize: () => number;
    overscan?: number;
}) {
    // Well under WebKit's hard 33,554,432 px limit
    const MAX_SCROLL_HEIGHT = 30_000_000;

    let scrollTop = $state(0);
    let containerHeight = $state(600);

    const rowHeight = $derived(options.estimateSize());
    const totalCount = $derived(options.count());
    const overscan = $derived(options.overscan ?? 10);

    // True pixel height if every row were rendered at full size
    const trueTotalHeight = $derived(totalCount * rowHeight);
    const needsScaling = $derived(trueTotalHeight > MAX_SCROLL_HEIGHT);
    const headerHeight = 34; // Sticky header height (must match padding-top in CsvTableBody)
    const virtualTotalHeight = $derived(
        (needsScaling ? MAX_SCROLL_HEIGHT : trueTotalHeight) + headerHeight,
    );

    // Observe the scroll element
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

        // Seed initial values
        containerHeight = el.clientHeight;
        scrollTop = el.scrollTop;

        return () => {
            observer.disconnect();
            el.removeEventListener("scroll", onScroll);
        };
    });

    // ---------- Core: map scroll position → visible row window ----------
    const virtualItems = $derived.by(() => {
        if (totalCount === 0) return [];

        // Reduce available visual container size by the header so you don't overcount rows
        const visibleCount = Math.ceil(containerHeight / rowHeight);

        let firstVisibleRow: number;

        if (!needsScaling) {
            // Normal mode – direct pixel mapping
            // Offset scroll by header explicitly so row 0 starts after header
            firstVisibleRow = Math.floor(Math.max(0, scrollTop - headerHeight) / rowHeight);
        } else {
            // Scaled mode – use scroll fraction
            const maxScroll = Math.max(1, virtualTotalHeight - containerHeight);
            const scrollableRange = Math.max(1, maxScroll - headerHeight);
            const fraction = Math.min(1, Math.max(0, scrollTop - headerHeight) / scrollableRange);
            const maxFirstRow = Math.max(0, totalCount - visibleCount);
            firstVisibleRow = Math.round(fraction * maxFirstRow);
        }

        // Clamp
        firstVisibleRow = Math.max(
            0,
            Math.min(firstVisibleRow, Math.max(0, totalCount - visibleCount)),
        );

        // Apply overscan
        const startIdx = Math.max(0, firstVisibleRow - overscan);
        const endIdx = Math.min(
            totalCount - 1,
            firstVisibleRow + visibleCount + overscan,
        );

        // Position the block so the first *visible* row lines up with scrollTop
        // when unscaled. When scaled, we calculate its intended pixel start relative 
        // to the real scrolled position so items compress correctly.
        const blockStart = !needsScaling
            ? startIdx * rowHeight
            : Math.max(0, scrollTop - headerHeight) -
            (firstVisibleRow - startIdx) * rowHeight;

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
                let top = index * rowHeight;
                if (scrollOpts?.align === "center") {
                    top -= containerHeight / 2;
                } else if (scrollOpts?.align === "end") {
                    top -= containerHeight - rowHeight;
                }
                el.scrollTo({ top: Math.max(0, top) });
            } else {
                const visibleCount = Math.ceil(containerHeight / rowHeight);
                const maxFirstRow = Math.max(1, totalCount - visibleCount);
                const fraction = Math.min(1, index / maxFirstRow);
                const maxScroll = virtualTotalHeight - containerHeight;
                el.scrollTo({ top: fraction * maxScroll });
            }
        },
    };
}
