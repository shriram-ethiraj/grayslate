import { createThemeConfig } from './theme-factory';

export const andromedaConfig = createThemeConfig('dark', {
    background: '#1b1e26', // base00
    foreground: '#e4dff0', // base01
    caret: '#ffffff', // base04
    selection: '#2a5f88',              // pure blue — clearly "primary selection"; less teal than before
    lineHighlight: '#ffffff12', // neutral white ~7% — reduced 35% from #ffffff1c; softer but still perceptible
    searchMatch: '#ffdd8055',         // golden yellow tint — all search matches
    searchMatchSelected: '#ffdd80cc', // bright gold — currently focused search match
    selectionMatch: '#7dd3fc20',      // sky-300 ~13% fill — 1.33:1; hue 199° sits in the free gap between teal-variable (170°) and selection-blue (206°)
    selectionMatchBorder: '#7dd3fc70',// sky-300 ~44% border — 3.00:1 (meets WCAG non-text); reads as cool sky, not purple, not green
    gutterBackground: '#1e212b', // darkBackground
    gutterForeground: '#748099',

    keyword: '#d667ff', // base05
    variable: '#24e3c3', // base06
    property: '#24e3c3', // base06
    type: '#ffdd80', // base07
    namespace: '#6ae4b9', // base0E
    operator: '#c7c7ff', // base0D
    punctuation: '#c7c7ff', // base0D
    function: '#ffdd80', // base07
    number: '#ff7057', // base09
    annotation: '#3c94ff', // base0F
    string: '#a6e07a', // base08
    comment: '#a8aab9', // base0A
    tagName: '#fd3681', // base0C
    attributeName: '#3c94ff', // base0F
    heading: '#ff40b3', // base0B
    strong: '#ff7057', // base09
    emphasis: '#6ae4b9', // base0E
    link: '#3c94ff', // base0F
    url: '#6ae4b9', // base0E
    invalid: '#ff3162',
});
