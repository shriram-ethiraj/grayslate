/**
 * Keyboard / mouse list-navigation logic for the library sidebar.
 *
 * Encapsulates highlight tracking, keyboard ArrowUp/Down/Enter handling,
 * viewport-aware snapping, scroll-into-view, and the pointer-move guard
 * that prevents scroll-induced hover from hijacking keyboard focus.
 *
 * Must be instantiated during component initialization (uses $state,
 * $derived, $effect internally).
 */
import { tick } from "svelte";
import type { LibraryFileRecord } from "$lib/files/sidebarUtils";
import type { RecentFileSource } from "$lib/files/recentFiles";

export interface ListNavigatorConfig {
    /** Reactive getter — called inside $derived to track the current result list. */
    getActiveResults: () => LibraryFileRecord[];
    /** Getter for the scrollable container DOM element. */
    getScrollContainer: () => HTMLElement | null;
    /** Called when the user presses Enter on the highlighted item. */
    onOpen: (path: string, source: RecentFileSource) => void;
}

export function useListNavigator(config: ListNavigatorConfig) {
    // -----------------------------------------------------------------------
    // State
    // -----------------------------------------------------------------------

    let highlightedIndex = $state(0);

    // Plain boolean — not reactive state. Toggled synchronously by keyboard
    // handlers and the pointermove listener; never drives Svelte re-renders.
    let isKeyboardNavigating = false;

    // -----------------------------------------------------------------------
    // Derived
    // -----------------------------------------------------------------------

    const highlightedPath = $derived.by(() => {
        const results = config.getActiveResults();
        if (results.length === 0) return undefined;
        return results[Math.min(highlightedIndex, results.length - 1)]?.path;
    });

    // -----------------------------------------------------------------------
    // Pointer-move guard
    //
    // When ArrowUp/Down scrolls the list, items slide under a stationary
    // cursor and fire `mouseenter`. Without this guard, every such event
    // would hijack `highlightedIndex` away from the keyboard target.
    //
    // `pointermove` only fires when the cursor physically moves — not when
    // elements scroll underneath it — so it's the correct signal to re-enable
    // hover-driven highlighting.
    // -----------------------------------------------------------------------

    $effect(() => {
        const container = config.getScrollContainer();
        if (!container) return;

        function onPointerMove(): void {
            isKeyboardNavigating = false;
        }

        container.addEventListener("pointermove", onPointerMove, { passive: true });
        return () => container.removeEventListener("pointermove", onPointerMove);
    });

    // -----------------------------------------------------------------------
    // Private viewport helpers
    // -----------------------------------------------------------------------

    /** Scroll the `[data-sidebar-highlighted]` element into view if it's outside the container. */
    async function scrollHighlightedIntoView(): Promise<void> {
        await tick();
        const container = config.getScrollContainer();
        const el = container?.querySelector<HTMLElement>("[data-sidebar-highlighted]");
        el?.scrollIntoView({ block: "nearest", behavior: "auto" });
    }

    /** Is the currently highlighted card at least partially visible? */
    function isHighlightedVisible(): boolean {
        const container = config.getScrollContainer();
        if (!container) return false;
        const el = container.querySelector<HTMLElement>("[data-sidebar-highlighted]");
        if (!el) return false;
        const cr = container.getBoundingClientRect();
        const er = el.getBoundingClientRect();
        return er.bottom > cr.top && er.top < cr.bottom;
    }

    /** Index of the first card (even partially) within the scroll viewport. */
    function findFirstVisibleCardIndex(): number {
        const container = config.getScrollContainer();
        if (!container) return 0;
        const results = config.getActiveResults();
        const cr = container.getBoundingClientRect();
        for (const card of container.querySelectorAll<HTMLElement>("[data-card-path]")) {
            const r = card.getBoundingClientRect();
            if (r.bottom > cr.top && r.top < cr.bottom) {
                const path = card.getAttribute("data-card-path");
                if (path) {
                    const idx = results.findIndex((f) => f.path === path);
                    if (idx !== -1) return idx;
                }
            }
        }
        return 0;
    }

    /** Index of the last card (even partially) within the scroll viewport. */
    function findLastVisibleCardIndex(): number {
        const results = config.getActiveResults();
        const container = config.getScrollContainer();
        if (!container) return Math.max(0, results.length - 1);
        const cr = container.getBoundingClientRect();
        let lastIdx = Math.max(0, results.length - 1);
        for (const card of container.querySelectorAll<HTMLElement>("[data-card-path]")) {
            const r = card.getBoundingClientRect();
            if (r.bottom > cr.top && r.top < cr.bottom) {
                const path = card.getAttribute("data-card-path");
                if (path) {
                    const idx = results.findIndex((f) => f.path === path);
                    if (idx !== -1) lastIdx = idx;
                }
            }
        }
        return lastIdx;
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    return {
        /** Path of the currently highlighted card (reactive). */
        get highlightedPath() {
            return highlightedPath;
        },

        /** Reset highlight to the first item (e.g. on new results or query clear). */
        reset(): void {
            highlightedIndex = 0;
        },

        /** Scroll the list container to the top (e.g. on filter/sort change). */
        scrollToTop(): void {
            config.getScrollContainer()?.scrollTo({ top: 0, behavior: "auto" });
        },

        /**
         * Called from `onmouseenter` on file cards.
         * Updates the highlight unless keyboard navigation is active.
         */
        handleHighlight(path: string): void {
            if (isKeyboardNavigating) return;
            const results = config.getActiveResults();
            const idx = results.findIndex((r) => r.path === path);
            if (idx !== -1) {
                highlightedIndex = idx;
            }
        },

        /**
         * Keyboard handler for ArrowUp / ArrowDown / Enter.
         * Wire to the search input's `keydown` event.
         */
        handleKeydown(event: KeyboardEvent): void {
            const results = config.getActiveResults();
            const count = results.length;
            if (count === 0) return;

            switch (event.key) {
                case "ArrowDown":
                    event.preventDefault();
                    isKeyboardNavigating = true;
                    // If the highlighted item has scrolled out of view (user
                    // scrolled manually), snap to the first visible card instead
                    // of jumping from an off-screen position.
                    if (!isHighlightedVisible()) {
                        highlightedIndex = findFirstVisibleCardIndex();
                    } else {
                        highlightedIndex = Math.min(highlightedIndex + 1, count - 1);
                    }
                    void scrollHighlightedIntoView();
                    break;
                case "ArrowUp":
                    event.preventDefault();
                    isKeyboardNavigating = true;
                    if (!isHighlightedVisible()) {
                        highlightedIndex = findLastVisibleCardIndex();
                    } else {
                        highlightedIndex = Math.max(highlightedIndex - 1, 0);
                    }
                    void scrollHighlightedIntoView();
                    break;
                case "Enter":
                    event.preventDefault();
                    {
                        const idx = Math.min(highlightedIndex, count - 1);
                        const file = results[idx];
                        if (file) {
                            config.onOpen(file.path, file.source);
                        }
                    }
                    break;
            }
        },
    };
}

export type ListNavigator = ReturnType<typeof useListNavigator>;
