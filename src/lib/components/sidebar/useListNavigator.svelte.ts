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
import type { HotkeyBinding } from "$lib/hotkeys";

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
    // Private action functions — stable references used in hotkey bindings
    // -----------------------------------------------------------------------

    function navigateDown(e: KeyboardEvent): void {
        const results = config.getActiveResults();
        const count = results.length;
        if (count === 0) return;
        e.preventDefault();
        isKeyboardNavigating = true;
        // If the highlighted item has scrolled out of view (user scrolled
        // manually), snap to the first visible card instead of jumping from
        // an off-screen position.
        if (!isHighlightedVisible()) {
            highlightedIndex = findFirstVisibleCardIndex();
        } else {
            highlightedIndex = Math.min(highlightedIndex + 1, count - 1);
        }
        void scrollHighlightedIntoView();
        const file = results[Math.min(highlightedIndex, count - 1)];
        if (file) config.onOpen(file.path, file.source);
    }

    function navigateUp(e: KeyboardEvent): void {
        const results = config.getActiveResults();
        const count = results.length;
        if (count === 0) return;
        e.preventDefault();
        isKeyboardNavigating = true;
        if (!isHighlightedVisible()) {
            highlightedIndex = findLastVisibleCardIndex();
        } else {
            highlightedIndex = Math.max(highlightedIndex - 1, 0);
        }
        void scrollHighlightedIntoView();
        const file = results[Math.min(highlightedIndex, count - 1)];
        if (file) config.onOpen(file.path, file.source);
    }

    function openHighlighted(e: KeyboardEvent): void {
        const results = config.getActiveResults();
        const count = results.length;
        if (count === 0) return;
        e.preventDefault();
        const file = results[Math.min(highlightedIndex, count - 1)];
        if (file) config.onOpen(file.path, file.source);
    }

    // -----------------------------------------------------------------------
    // TanStack hotkey bindings
    //
    // Two sets are exposed so callers can attach them to the right elements:
    //
    //   inputHotkeys  — for <input> / <textarea> targets where ignoreInputs
    //                   must be false (the target IS the input).
    //
    //   listHotkeys   — for the scroll-container <div> where the buttons are
    //                   not inputs, so the default ignoreInputs: true is correct
    //                   (prevents firing when a hypothetical child input is focused).
    // -----------------------------------------------------------------------

    /** Wire these to the search <input> via `use:hotkey`. */
    const inputHotkeys: HotkeyBinding[] = [
        { key: "ArrowDown", callback: navigateDown, options: { ignoreInputs: false, preventDefault: true } },
        { key: "ArrowUp",   callback: navigateUp,   options: { ignoreInputs: false, preventDefault: true } },
        { key: "Enter",     callback: openHighlighted, options: { ignoreInputs: false, preventDefault: true } },
    ];

    /** Wire these to the file-list scroll-container <div> via `use:hotkey`. */
    const listHotkeys: HotkeyBinding[] = [
        { key: "ArrowDown", callback: navigateDown, options: { ignoreInputs: true, preventDefault: true } },
        { key: "ArrowUp",   callback: navigateUp,   options: { ignoreInputs: true, preventDefault: true } },
        { key: "Enter",     callback: openHighlighted, options: { ignoreInputs: true, preventDefault: true } },
    ];

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    return {
        /** Path of the currently highlighted card (reactive). */
        get highlightedPath() {
            return highlightedPath;
        },

        /**
         * TanStack hotkey bindings for the search `<input>` (use `use:hotkey`).
         * `ignoreInputs: false` because the target element IS the input.
         */
        get inputHotkeys(): HotkeyBinding[] {
            return inputHotkeys;
        },

        /**
         * TanStack hotkey bindings for the file-list scroll container (use `use:hotkey`).
         * `ignoreInputs: true` so keys don't double-fire when the search input is focused.
         */
        get listHotkeys(): HotkeyBinding[] {
            return listHotkeys;
        },

        /** Reset highlight to the first item (e.g. on new results or query clear). */
        reset(): void {
            highlightedIndex = 0;
        },

        /**
         * Reset highlight, preferring `path` if it exists in the current results.
         * Falls back to index 0 when the path is absent or undefined.
         * Use this after tab/filter/sort changes so the currently open file
         * stays highlighted when it is visible in the new result set.
         */
        resetToFile(path: string | undefined): void {
            if (path) {
                const results = config.getActiveResults();
                const idx = results.findIndex((r) => r.path === path);
                if (idx !== -1) {
                    highlightedIndex = idx;
                    return;
                }
            }
            highlightedIndex = 0;
        },
        focusHighlight(path: string): void {
            const results = config.getActiveResults();
            const idx = results.findIndex((r) => r.path === path);
            if (idx !== -1) {
                highlightedIndex = idx;
            }
        },

        /** Highlight a result and bring its card into the visible viewport. */
        revealHighlight(path: string): void {
            const results = config.getActiveResults();
            const idx = results.findIndex((r) => r.path === path);
            if (idx === -1) return;

            highlightedIndex = idx;
            void scrollHighlightedIntoView();
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
    };
}

export type ListNavigator = ReturnType<typeof useListNavigator>;
