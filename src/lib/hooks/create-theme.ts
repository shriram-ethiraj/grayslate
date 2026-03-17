import { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import type { TagStyle } from '@codemirror/language';

export interface ThemeSettings {
    /**
     * Editor background.
     */
    background: string;

    /**
     * Default text color.
     */
    foreground: string;

    /**
     * Caret color.
     */
    caret: string;

    /**
     * Selection background.
     */
    selection: string;

    /**
     * Background of highlighted lines.
     */
    lineHighlight: string;

    /**
     * Background for non-active search/find matches.
     */
    searchMatch: string;

    /**
     * Background for the currently focused search/find match.
     */
    searchMatchSelected: string;

    /**
     * Background fill for other occurrences of the selected word.
     * Should be a faint tint — the border provides the primary visual signal.
     */
    selectionMatch: string;

    /**
     * Outline/border color for other occurrences of the selected word.
     * Should be clearly visible and a different hue from `selection` so the
     * two states are instantly distinguishable without relying on opacity alone.
     */
    selectionMatchBorder: string;

    /**
     * Gutter background.
     */
    gutterBackground: string;

    /**
     * Text color inside gutter.
     */
    gutterForeground: string;
}

export interface ThemeConfig {
    /**
     * Theme variant. Determines which styles CodeMirror will apply by default.
     */
    variant: 'light' | 'dark';
    /**
     * Settings to customize the look of the editor, like background, gutter, selection and others.
     */
    settings: ThemeSettings;
    /**
     * Syntax highlighting styles.
     */
    styles: TagStyle[];
}

// ---------------------------------------------------------------------------
// Why `&.cm-editor` instead of bare `.cm-foo`?
//
// EditorView.theme() scopes every selector with a unique class prefix, e.g.
//   `.cm-activeLine`  →  `.<prefix> .cm-activeLine`   specificity (0,2,0)
//
// CM6 base themes (search, active-line, gutters …) use `&dark` / `&light`
// which expand to a *different* single class:
//   `&dark .cm-activeLine`  →  `.<darkID> .cm-activeLine`  specificity (0,2,0)
//
// Equal specificity means the winner depends on CSS insertion order, which is
// fragile across bundlers and WebView runtimes.  By using `&.cm-editor` we
// create a compound selector on the wrapper element (which always carries
// both the theme class and `.cm-editor`):
//   `&.cm-editor .cm-activeLine`  →  `.<prefix>.cm-editor .cm-activeLine`
//                                      specificity (0,3,0)  — always wins.
//
// Comma-separated selectors are split into individual entries because
// style-mod's finish() only replaces the first `&` occurrence.
// ---------------------------------------------------------------------------

export const createTheme = (config: ThemeConfig): Extension => {
    // Propagate the search-match color to a CSS variable so non-editor UI
    // (e.g. sidebar search highlights) stays in sync with the active theme.
    // The guard makes this SSR-safe for the SvelteKit static adapter.
    if (typeof document !== 'undefined') {
        document.documentElement.style.setProperty('--search-match-bg', config.settings.searchMatch);
        // Propagate selectionMatch colors so non-editor UI (sidebar text matches)
        // stays visually consistent with the editor's word-occurrence style.
        document.documentElement.style.setProperty('--selection-match-bg', config.settings.selectionMatch);
        document.documentElement.style.setProperty('--selection-match-border', config.settings.selectionMatchBorder);
    }

    const theme = EditorView.theme(
        {
            // Editor wrapper — background & foreground
            // eslint-disable-next-line @typescript-eslint/naming-convention
            '&': {
                backgroundColor: config.settings.background,
                color: config.settings.foreground,
            },

            // Caret
            '&.cm-editor .cm-content': {
                caretColor: config.settings.caret,
            },
            '&.cm-editor .cm-cursor': {
                borderLeftColor: config.settings.caret,
            },
            '&.cm-editor .cm-dropCursor': {
                borderLeftColor: config.settings.caret,
            },

            // Selection — focused state (custom draw-selection layer)
            '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground': {
                backgroundColor: config.settings.selection,
            },
            // Selection — focused state catch-all
            '&.cm-focused .cm-selectionBackground': {
                backgroundColor: config.settings.selection,
            },
            // Selection — unfocused state
            '&.cm-editor .cm-selectionBackground': {
                backgroundColor: config.settings.selection,
            },
            // Native browser selection highlight
            '&.cm-editor .cm-content ::selection': {
                backgroundColor: config.settings.selection,
            },

            // Active line highlight
            '&.cm-editor .cm-activeLine': {
                backgroundColor: config.settings.lineHighlight,
            },

            // Find / search matches
            '&.cm-editor .cm-searchMatch': {
                backgroundColor: config.settings.searchMatch,
                outline: '1px solid ' + config.settings.searchMatch,
            },
            '&.cm-editor .cm-searchMatch.cm-searchMatch-selected': {
                backgroundColor: config.settings.searchMatchSelected,
                outline: '1px solid ' + config.settings.searchMatchSelected,
            },

            // Other occurrences of the currently selected word.
            // A faint fill + visible outline in a different hue from `selection`
            // creates the classic "word occurrence box" found in VS Code / JetBrains.
            '&.cm-editor .cm-selectionMatch': {
                backgroundColor: config.settings.selectionMatch,
                outline: '1px solid ' + config.settings.selectionMatchBorder,
                borderRadius: '2px',
            },

            // Gutters
            '&.cm-editor .cm-gutters': {
                backgroundColor: config.settings.gutterBackground,
                color: config.settings.gutterForeground,
            },
            '&.cm-editor .cm-activeLineGutter': {
                backgroundColor: config.settings.lineHighlight,
            },
        },
        {
            dark: config.variant === 'dark',
        },
    );

    const highlightStyle = HighlightStyle.define(config.styles);
    const extension = [theme, syntaxHighlighting(highlightStyle)];

    return extension;
};
