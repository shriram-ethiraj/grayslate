/**
 * UI preferences state — persists sidebar width and other layout preferences.
 *
 * The sidebar width is stored as a percentage of the total horizontal space
 * (matching paneforge's sizing model). The `open` flag tracks whether the
 * sidebar pane is currently expanded so we can restore the last-used width
 * on toggle.
 */

const DEFAULT_SIDEBAR_WIDTH = 20; // percentage of total width

export const uiState = $state<{
    sidebar: {
        open: boolean;
        /** Last user-set width as a percentage (paneforge units). */
        width: number;
    };
}>({
    sidebar: {
        open: false,
        width: DEFAULT_SIDEBAR_WIDTH,
    },
});
