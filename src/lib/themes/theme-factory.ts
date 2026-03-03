import { tags as t } from '@lezer/highlight';
import type { ThemeConfig } from '../hooks/create-theme';

export interface ThemePalette {
    // Editor UI settings
    background: string;
    foreground: string;
    caret: string;
    selection: string;
    lineHighlight: string;
    searchMatch: string;
    searchMatchSelected: string;
    selectionMatch: string;
    gutterBackground: string;
    gutterForeground: string;

    // Code syntax highlighting colors
    keyword: string;
    variable: string;
    property: string;
    type: string;
    namespace: string;
    operator: string;
    punctuation: string;
    function: string;
    number: string;
    annotation: string;
    string: string;
    comment: string;
    tagName: string;
    attributeName: string;
    heading: string;
    strong: string;
    emphasis: string;
    link: string;
    url: string;
    invalid: string;
}

export function createThemeConfig(
    variant: 'light' | 'dark',
    palette: ThemePalette
): ThemeConfig {
    return {
        variant,
        settings: {
            background: palette.background,
            foreground: palette.foreground,
            caret: palette.caret,
            selection: palette.selection,
            lineHighlight: palette.lineHighlight,
            searchMatch: palette.searchMatch,
            searchMatchSelected: palette.searchMatchSelected,
            selectionMatch: palette.selectionMatch,
            gutterBackground: palette.gutterBackground,
            gutterForeground: palette.gutterForeground,
        },
        styles: [
            // Keywords and control flow
            { tag: t.keyword, color: palette.keyword, fontWeight: 'bold' },
            { tag: t.controlKeyword, color: palette.keyword, fontWeight: 'bold' },
            { tag: t.moduleKeyword, color: palette.keyword, fontWeight: 'bold' },

            // Names and variables
            { tag: [t.name, t.deleted, t.character, t.macroName], color: palette.variable },
            { tag: [t.variableName], color: palette.variable },
            { tag: [t.propertyName], color: palette.property, fontStyle: 'normal' },

            // Classes and types
            { tag: [t.typeName], color: palette.type },
            { tag: [t.className], color: palette.type, fontStyle: 'italic' },
            { tag: [t.namespace], color: palette.namespace, fontStyle: 'italic' },

            // Operators and punctuation
            { tag: [t.operator, t.operatorKeyword], color: palette.operator },
            { tag: [t.bracket], color: palette.punctuation },
            { tag: [t.brace], color: palette.punctuation },
            { tag: [t.punctuation], color: palette.punctuation },

            // Functions and parameters
            { tag: [t.function(t.variableName), t.labelName], color: palette.function },
            { tag: [t.definition(t.variableName)], color: palette.variable },

            // Constants and literals
            { tag: t.number, color: palette.number },
            { tag: t.changed, color: palette.number },
            { tag: t.annotation, color: palette.annotation, fontStyle: 'italic' },
            { tag: t.modifier, color: palette.annotation, fontStyle: 'italic' },
            { tag: t.self, color: palette.number },
            { tag: [t.color, t.constant(t.name), t.standard(t.name)], color: palette.number },
            { tag: [t.atom, t.bool, t.special(t.variableName)], color: palette.number },

            // Strings and regex
            { tag: [t.processingInstruction, t.inserted], color: palette.string },
            { tag: [t.special(t.string), t.regexp], color: palette.string },
            { tag: t.string, color: palette.string },

            // Punctuation and structure
            { tag: t.definition(t.typeName), color: palette.type, fontWeight: 'bold' },

            // Comments and documentation
            { tag: t.meta, color: palette.comment },
            { tag: t.comment, fontStyle: 'italic', color: palette.comment },
            { tag: t.docComment, fontStyle: 'italic', color: palette.comment },

            // HTML/XML elements
            { tag: [t.tagName], color: palette.tagName },
            { tag: [t.attributeName], color: palette.attributeName },

            // Markdown and text formatting
            { tag: [t.heading], fontWeight: 'bold', color: palette.heading },
            { tag: [t.strong], fontWeight: 'bold', color: palette.strong },
            { tag: [t.emphasis], fontStyle: 'italic', color: palette.emphasis },

            // Links and URLs
            {
                tag: [t.link],
                color: palette.link,
                fontWeight: '500',
                textDecoration: 'underline',
                textUnderlinePosition: 'under',
            },
            {
                tag: [t.url],
                color: palette.url,
                textDecoration: 'underline',
                textUnderlineOffset: '2px',
            },

            // Special states
            { tag: [t.invalid], color: palette.invalid },
            { tag: [t.strikethrough], color: palette.invalid, textDecoration: 'line-through' },

            // Enhanced syntax highlighting
            { tag: t.constant(t.name), color: palette.number },
            { tag: t.deleted, color: palette.invalid },
            { tag: t.squareBracket, color: palette.punctuation },
            { tag: t.angleBracket, color: palette.punctuation },
        ],
    };
}
