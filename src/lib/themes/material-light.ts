import { createThemeConfig } from './theme-factory';

export const materialLightConfig = createThemeConfig('light', {
    background: '#ffffff', // base00
    foreground: '#212121', // base02
    caret: '#9e9e9e', // base04
    selection: '#c5ddff',              // saturated cornflower blue — clearly "primary selection"
    lineHighlight: '#00000013', // neutral black ~7.5% — reduced 35% from #0000001e; softer but still perceptible
    searchMatch: '#ff980055',         // orange tint — all search matches
    searchMatchSelected: '#ff9800cc', // bright orange — currently focused search match
    selectionMatch: '#4a90d918',      // cornflower blue ~10% — same family as selection, VS Code-style "related occurrence" pattern
    selectionMatchBorder: '#4a90d960',// cornflower blue ~38% — crisp medium-blue border; pale selection bg + blue border box = immediately distinct
    gutterBackground: '#fafafa', // base07
    gutterForeground: '#757575', // base03

    keyword: '#00acc1', // base0D
    variable: '#424242', // base05
    property: '#00897b', // base11
    type: '#ff9800', // base0C
    namespace: '#3949ab', // base0E
    operator: '#3949ab', // base0E 
    punctuation: '#8e24aa', // base0F
    function: '#ff3e00', // base09
    number: '#ff9800', // base0C
    annotation: '#f44336', // base08
    string: '#43a047', // base10
    comment: '#757575', // base03
    tagName: '#ff3e00', // base09
    attributeName: '#424242', // base05
    heading: '#00897b', // base11
    strong: '#3949ab', // base0E
    emphasis: '#ff9800', // base0C
    link: '#8e24aa', // base0F
    url: '#00acc1', // base0D
    invalid: '#f44336', // base08
});
