import { debouncedSaveSetting, saveSetting } from "$lib/state/appSettings.svelte";

const DEFAULT_SIDEBAR_WIDTH = 20;

export const uiState = $state<{
    sidebar: {
        open: boolean;
        width: number;
    };
}>({
    sidebar: {
        open: false,
        width: DEFAULT_SIDEBAR_WIDTH,
    },
});

export function setSidebarWidth(width: number): void {
    const clamped = Math.max(15, Math.min(30, Math.round(width)));
    uiState.sidebar.width = clamped;
    debouncedSaveSetting("sidebar_width", String(clamped));
}

export function setSidebarOpen(open: boolean): void {
    uiState.sidebar.open = open;
    saveSetting("sidebar_open", String(open));
}
