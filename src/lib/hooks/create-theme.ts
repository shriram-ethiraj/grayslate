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

export const createTheme = (config: ThemeConfig): Extension => {
    const theme = EditorView.theme(
        {
            // eslint-disable-next-line @typescript-eslint/naming-convention
            '&': {
                backgroundColor: config.settings.background,
                color: config.settings.foreground,
            },
            '.cm-content': {
                caretColor: config.settings.caret,
            },
            '.cm-cursor, .cm-dropCursor': {
                borderLeftColor: config.settings.caret,
            },
            '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection':
            {
                backgroundColor: config.settings.selection,
            },
            '.cm-activeLine': {
                backgroundColor: config.settings.lineHighlight,
            },
            '.cm-gutters': {
                backgroundColor: config.settings.gutterBackground,
                color: config.settings.gutterForeground,
            },
            '.cm-activeLineGutter': {
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
