import { tags as t } from '@lezer/highlight';
import type { ThemeConfig } from '../hooks/create-theme';

// Base colors
const base00 = '#1b1e26'; // Background
const base01 = '#e4dff0'; // Foreground
const base02 = '#db45a270'; // Selection
const base04 = '#ffffff'; // Cursor
const base05 = '#d667ff'; // Keyword, Storage
const base06 = '#24e3c3'; // Variable, Parameter
const base07 = '#ffdd80'; // Function, Type, Class
const base08 = '#a6e07a'; // String, RegExp
const base09 = '#ff7057'; // Constant, Number
const base0A = '#a8aab9'; // Comment
const base0B = '#ff40b3'; // Heading
const base0C = '#fd3681'; // Tag
const base0D = '#c7c7ff'; // brackets/punctuation
const base0E = '#6ae4b9'; // special elements
const base0F = '#3c94ff'; // attributes and links
const invalid = '#ff3162';

// UI-specific colors
const darkBackground = '#242830';
const highlightBackground = '#30343d40';

export const andromedaConfig: ThemeConfig = {
    variant: 'dark',
    settings: {
        background: base00,
        foreground: base01,
        caret: base04,
        selection: base02,
        lineHighlight: highlightBackground,
        gutterBackground: darkBackground,
        gutterForeground: '#748099',
    },
    styles: [
        // Keywords and control flow
        { tag: t.keyword, color: base05, fontWeight: 'bold' },
        { tag: t.controlKeyword, color: base05, fontWeight: 'bold' },
        { tag: t.moduleKeyword, color: base05, fontWeight: 'bold' },

        // Names and variables
        { tag: [t.name, t.deleted, t.character, t.macroName], color: base06 },
        { tag: [t.variableName], color: base06 },
        { tag: [t.propertyName], color: base06, fontStyle: 'normal' },

        // Classes and types
        { tag: [t.typeName], color: base07 },
        { tag: [t.className], color: base07, fontStyle: 'italic' },
        { tag: [t.namespace], color: base0E, fontStyle: 'italic' },

        // Operators and punctuation
        { tag: [t.operator, t.operatorKeyword], color: base0D },
        { tag: [t.bracket], color: base0D },
        { tag: [t.brace], color: base0D },
        { tag: [t.punctuation], color: base0D },

        // Functions and parameters
        { tag: [t.function(t.variableName), t.labelName], color: base07 },
        { tag: [t.definition(t.variableName)], color: base06 },

        // Constants and literals
        { tag: t.number, color: base09 },
        { tag: t.changed, color: base09 },
        { tag: t.annotation, color: base0F, fontStyle: 'italic' },
        { tag: t.modifier, color: base0F, fontStyle: 'italic' },
        { tag: t.self, color: base09 },
        { tag: [t.color, t.constant(t.name), t.standard(t.name)], color: base09 },
        { tag: [t.atom, t.bool, t.special(t.variableName)], color: base09 },

        // Strings and regex
        { tag: [t.processingInstruction, t.inserted], color: base08 },
        { tag: [t.special(t.string), t.regexp], color: base08 },
        { tag: t.string, color: base08 },

        // Punctuation and structure
        { tag: t.definition(t.typeName), color: base07, fontWeight: 'bold' },

        // Comments and documentation
        { tag: t.meta, color: base0A },
        { tag: t.comment, fontStyle: 'italic', color: base0A },
        { tag: t.docComment, fontStyle: 'italic', color: base0A },

        // HTML/XML elements
        { tag: [t.tagName], color: base0C },
        { tag: [t.attributeName], color: base0F },

        // Markdown and text formatting
        { tag: [t.heading], fontWeight: 'bold', color: base0B },
        { tag: [t.strong], fontWeight: 'bold', color: base09 },
        { tag: [t.emphasis], fontStyle: 'italic', color: base0E },

        // Links and URLs
        {
            tag: [t.link],
            color: base0F,
            fontWeight: '500',
            textDecoration: 'underline',
            textUnderlinePosition: 'under',
        },
        {
            tag: [t.url],
            color: base0E,
            textDecoration: 'underline',
            textUnderlineOffset: '2px',
        },

        // Special states
        { tag: [t.invalid], color: invalid, textDecoration: 'underline wavy' },
        { tag: [t.strikethrough], color: invalid, textDecoration: 'line-through' },

        // Enhanced syntax highlighting
        { tag: t.constant(t.name), color: base09 },
        { tag: t.deleted, color: invalid },
        { tag: t.squareBracket, color: base0D },
        { tag: t.angleBracket, color: base0D },
    ],
};
