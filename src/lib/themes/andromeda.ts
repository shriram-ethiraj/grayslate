import { createThemeConfig } from './theme-factory';

export const andromedaConfig = createThemeConfig('dark', {
    background: '#1b1e26', // base00
    foreground: '#e4dff0', // base01
    caret: '#ffffff', // base04
    selection: '#db45a270', // base02
    lineHighlight: '#30343d40', // highlightBackground
    gutterBackground: '#242830', // darkBackground
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
