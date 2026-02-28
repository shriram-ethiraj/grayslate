# Editor Extensions

This document describes all custom CodeMirror extensions and editor utilities in `src/lib/editor/extensions/`.

---

## `stickyHeader.ts`

**Type:** CodeMirror extension factory + base theme

Provides a reusable top‑panel that behaves like a "sticky header". The
panel is initially hidden and only becomes visible once a designated
"anchor" line scrolls out of view; it hides again when the line returns
onscreen. It also keeps its content aligned with the editable text area by
shadowing the gutter width.

The implementation is intentionally minimal and highly performant:

* a passive `scroll` listener on `view.scrollDOM` toggles `display`
* visibility check uses `scrollTop` vs `lineBlockAt().bottom` (no layout)
* content rendering occurs only when the consumer's `shouldRerender`
  callback returns `true` (usually just a string equality test)
* gutter padding sync reads `contentDOM.offsetLeft` (composited style)

### Usage example (csvRainbowHighlight)

```ts
import { createStickyHeaderPanel, stickyHeaderBaseTheme } from "./stickyHeader";

const csvStickyHeader = createStickyHeaderPanel({
  class: "csv-sticky-header",
  anchorLine: 1,
  render(dom, view) { /* paint header row */ },
  shouldRerender(update) { /* compare line‑1 text */ }
});

// then include `csvStickyHeader` and `stickyHeaderBaseTheme` in
// the extension array for CSV files (see csvRainbowHighlight.ts).
```

### Future use

This utility will also power the JSON path breadcrumb that appears when
scrolling through large JSON documents; simply provide a different render
function and re-render predicate.

---

## `stickyScroll.ts`

**Type:** Generic CodeMirror extension factory + base theme

A language-agnostic sticky scroll engine.  Scope-opening lines pin to the top of the editor as the user scrolls, showing the nesting context at a glance — exactly like VS Code's sticky scroll.

### Architecture

The extension is parameterised by a **scope provider** — a function that returns which lines are "scopes" in the current document.  This makes it trivially extensible:

| Language   | Scope provider                              |
|------------|---------------------------------------------|
| JSON       | Object / Array opening lines (lezer tree)   |
| CSV        | Header row (line 1 → last line)             |
| Markdown   | Heading lines (`# …`, `## …`, …)           |
| XML / HTML | Element open-tag lines                      |
| YAML       | Mapping-key lines at increasing indent      |

### Line rendering — DOM cloning (Monaco-style)

Sticky-scroll rows are rendered by **deep-cloning the real `.cm-line` element** from the editor DOM — the same approach VS Code / Monaco uses.  This captures *all* decorations from *any* extension (ViewPlugin marks, HighlightStyle tags, widgets, …) with zero coupling to those extensions.

Clones are cached in a `Map<lineNumber, HTMLElement>` and proactively warmed on every viewport change.  When a line is not in the DOM (rare: direct jump), the engine falls back to `renderHighlightedLine` which walks the lezer syntax tree — producing identical output for any language with a lezer grammar.

### Usage

```ts
import { createStickyScroll } from "./stickyScroll";
import type { StickyScope, StickyScrollConfig } from "./stickyScroll";

const ext = createStickyScroll({
    class: "cm-json-sticky-scroll",
    computeScopes(view) {
        // walk the syntax tree, return StickyScope[]
        return [];
    },
    maxLines: 5,       // optional, default 5
});
// ext is a self-contained Extension (panel + base theme)
```

### Public API

| Export | Type | Description |
|---|---|---|
| `createStickyScroll(config)` | `(StickyScrollConfig) → Extension` | Factory — returns panel + theme |
| `stickyScrollBaseTheme` | `Extension` | Exported for advanced use (factory already includes it) |
| `StickyScope` | interface | `{ openLine: number, closeLine: number }` |
| `StickyScrollConfig` | interface | `{ class, computeScopes, maxLines?, shouldReparse? }` |

### Key internals

- **`findLineElement(view, lineNumber)`** — Locates the `.cm-line` DOM element via `view.domAtPos()`. Returns `null` when the line is outside the viewport buffer.
- **`cloneLineContent(source, container)`** — Deep-clones child nodes from a `.cm-line` into the sticky row.
- **`warmCloneCache(view, scopes, cache)`** — Proactively clones all scope-opening lines currently in the DOM.
- **`renderHighlightedLine(container, state, lineFrom, lineTo)`** — Lezer-based fallback. Renders a line with full syntax highlighting via `highlightTree` + `highlightingFor`.
- **`computeVisibleStack(view, scopes, maxLines)`** — Filters scopes by `scrollTop`.  Included when open line is above viewport top and close line is below. Returns at most `maxLines` entries (deepest kept).
- **`syncLayoutStyles(root, view, cache)`** — Syncs font metrics, gutter width, background, and hover colours from the editor DOM.  Cached — writes only on change.

### Rendering priority

1. **Live DOM clone** — line is in the viewport right now (most up-to-date)
2. **Cached clone** — line was in the viewport earlier (survives viewport changes)
3. **Lezer fallback** — line was never rendered (works perfectly for grammar-based languages)

### Performance

| Operation | Cost |
|---|---|
| Scope parsing | Consumer-defined; called only on `docChanged` or syntax-tree update |
| Stack computation | Linear scan; `lineBlockAt` is O(log n) from CM's height B-tree |
| Scroll handler | rAF-throttled passive listener; DOM writes only when stack changes |
| Line rendering | DOM `cloneNode(true)` — fast; lezer walk only as fallback |
| Clone cache warm | O(scopes × 1 querySelector) per viewport shift |
| Layout sync | Cached reads; writes only on change (theme switches) |

### Styling

Layout CSS custom properties (`--sticky-scroll-*`) are set at runtime.  Syntax colours come from the cloned DOM's own CSS classes — no colour rules needed in the base theme.

---

## `jsonStickyScroll.ts`

**Type:** JSON scope provider for `stickyScroll.ts`

Thin wrapper that provides JSON-specific scope detection and calls `createStickyScroll`.

### Scope detection

Walks the lezer JSON syntax tree (`syntaxTree(state).iterate`).  Only `Object` and `Array` nodes that span more than one line become scopes.

### Usage

```ts
import { jsonStickyScroll } from "./jsonStickyScroll";

// In the extension array for JSON:
case "json":
    return [json(), jsonStickyScroll, ...];
```

The export is a self-contained `Extension` — no separate theme import needed.

### Key function

- **`parseJsonScopes(view)`** — Returns a flat `StickyScope[]` in document order.  Each entry records the open/close line numbers of a multi-line Object or Array.

---

## `jsonFoldWidget.ts`

**Type:** CodeMirror extension (`Prec.highest`)

Replaces the default `…` fold placeholder with a context-aware inline summary of the collapsed JSON node.

### Behavior

| Collapsed node | Placeholder rendered |
|---|---|
| Array | `… 4` (item count) |
| Object | ` key1: val1, key2: val2, … ` (first 2 key-value pairs) |
| Other | `…` (fallback) |

### Configuration

| Constant | Value | Description |
|---|---|---|
| `MAX_PREVIEW_PAIRS` | `2` | Max key-value pairs shown in object preview |
| `MAX_VALUE_LEN` | `20` | Max characters per value before truncating with `…` |

### Key functions

- **`preparePlaceholder(state, range)`** — Walks the lezer syntax tree to identify the Array/Object node at the fold site and returns a typed `FoldInfo` descriptor. Called once per fold (cheap).
- **`placeholderDOM(view, onclick, prepared)`** — Builds the DOM `<span>` shown in place of folded text. Reuses the `.cm-foldPlaceholder` CSS class for base styling.

### Styling

The `.cm-foldPlaceholder` class is overridden in `layout.css` to match the `jsonInlayHints` visual style:

```css
background-color: rgba(128, 128, 128, 0.1);
color: var(--text-muted, #888);
font-size: 0.9em;
font-family: monospace;
padding: 0 4px;
border-radius: 3px;
```

Hover brightens the background to `rgba(128, 128, 128, 0.2)`.

---

## `jsonInlayHints.ts`

**Type:** CodeMirror `ViewPlugin`

Renders inline index badges before each element in a JSON array, similar to VS Code / IntelliJ inlay hints.

### Behavior

For a JSON array like:
```json
["apple", "banana", "cherry"]
```
Each element is preceded by a small monospace badge: `0`, `1`, `2`.

### Widget: `ArrayIndexWidget`

Renders a `<span class="cm-json-array-index">` with inline styles:

| Property | Value |
|---|---|
| Color | `var(--text-muted, #888)` |
| Font size | `0.9em` |
| Padding | `0px 4px` |
| Border radius | `3px` |
| Background | `rgba(128, 128, 128, 0.1)` |
| Margin right | `6px` |
| Pointer events | `none` (non-interactive) |

### Performance

- Decorations are rebuilt only when `docChanged`, `viewportChanged`, or the syntax tree changes.
- Only processes the visible viewport (`view.visibleRanges`), not the full document.
- Skips bracket, comma, and error (`⚠`) tokens when counting indices.

---

## `colorHints.ts`

**Type:** CodeMirror `ViewPlugin`

Renders an inline color swatch before any recognized color literal in the editor. Works across **all languages** — no syntax tree dependency.

### Supported color formats

| Format | Example |
|---|---|
| Hex 3-digit | `#f0a` |
| Hex 4-digit (alpha) | `#f0af` |
| Hex 6-digit | `#ff0044` |
| Hex 8-digit (alpha) | `#ff004480` |
| `rgb()` / `rgba()` | `rgb(255, 0, 68)` |
| `hsl()` / `hsla()` | `hsl(210deg 50% 60%)` |

Both legacy comma syntax and modern space/slash syntax are supported for functional colors.

### Widget: `ColorSwatchWidget`

Renders a small `<span class="cm-color-swatch">` box:

- `0.8em × 0.8em` inline block, `verticalAlign: middle`
- Background is the resolved CSS color
- A **checkerboard pattern** underlays the color to visualize alpha transparency
- Border uses `var(--border)` to match the app theme in both light/dark mode
- Non-interactive (`pointerEvents: none`)

### Validation

Colors are validated via `toCSSColor()` which uses the browser's Canvas 2D API to parse the color string. Invalid strings (e.g. `#xyz`) are silently skipped. Results are cached in a `Map<string, string | null>` capped at 2000 entries.

### Performance

- Scans only the visible viewport using a single regex pass per range.
- Updates on `docChanged` or `viewportChanged`.

---

## `autocompleteFactory.ts`

**Type:** Factory utilities for CodeMirror autocomplete providers

Provides two factory functions to create reusable, icon-aware autocomplete extensions.

### `createAutocompleteRenderer(iconHtml?, iconNode?, detailText?)`

Builds a custom `info` renderer for a `Completion` entry. Renders a flex row with:
- An optional **Lucide icon** (via `iconNode`) or raw HTML icon (`iconHtml`)
- An optional **detail label** (`detailText`) as a styled `<span>`

Colors intentionally inherit from the parent `<li>` (shadcn-styled autocomplete list) rather than being hardcoded, so hover state works correctly.

### `createAutocompleteProvider(config: AutocompleteConfig)`

Takes an `AutocompleteConfig` and returns a CodeMirror `CompletionSource` function. Each item is created as a `snippetCompletion` with tab-stop support.

```typescript
interface AutocompleteConfig {
    triggerRegex: RegExp;   // pattern to match before cursor to open completion
    validForRegex: RegExp;  // pattern the matched text must satisfy to keep open
    items: AutocompleteItem[];
}

interface AutocompleteItem {
    snippet: string;        // CodeMirror snippet string with ${} tab stops
    label: string;          // text shown in the list
    type?: string;          // completion type hint (e.g. "text", "keyword")
    iconNode?: IconNode;    // Lucide icon node
    iconHtml?: string;      // raw HTML fallback for icon
    detailText?: string;    // secondary description shown next to the icon
}
```

---

## `jsonKeyPath.ts`

**Type:** CodeMirror `hoverTooltip`

Shows the full dot-notation key path to any node when hovering anywhere in a JSON document.

### Behavior

Hovering over any key, value, or bracket in a JSON document displays a tooltip above the cursor containing the complete JSONPath from the root (`$`) to the hovered node:

| Hovered position | Tooltip shown |
|---|---|
| Value `10` in `[{"coordinates":[10,20]}]` | `$[0].coordinates[0]` |
| Key `"address"` in `{"user":{"address":{}}}` | `$.user.address` |
| Key `"id"` in `[{"id":1},{"id":2}]` | `$[0].id` / `$[1].id` |
| Opening `{` of first object in root array | `$[0]` |
| Top-level key | `$.name` |
| Root `{` or `[` | `$` |

### Path-building algorithm

Walks **up** the lezer syntax tree from the hovered position, prepending a segment at each ancestor:

1. **Ancestor is `Property`** — extract the key literal (strip quotes), prepend to path; then jump directly to the `Object`'s parent to skip the redundant `Object` node
2. **Ancestor is `Array`** — count non-structural siblings before this node to get the 0-based index, prepend `[n]`
3. **Anything else** (`Object`, `JsonText`, tokens) — skip upward without adding a segment

The result is always prefixed with `$` (JSONPath root). An empty path stays as `$`.

### Styling

The tooltip reuses the shadcn `--popover` / `--border` tokens for consistent theming in both light and dark mode.

The `.cm-json-key-path-tooltip` inner element:

| Property | Value |
|---|---|
| Font | monospace, `0.82em` |
| Padding | `3px 10px` |
| Color | `var(--text-muted, #888)` |
| User-select | `text` (copyable) |

---

## `markdown/markdownAutocomplete.ts`

**Type:** Slash-command autocomplete for the Markdown editor

Provides a `/command` autocomplete menu triggered by typing `/` in a Markdown document. Built on top of `autocompleteFactory`.

### Trigger

Typing `/` followed by optional word characters opens the popup. The list filters live as the user continues typing.

### Available commands

| Command | Snippet | Description |
|---|---|---|
| `/h1` – `/h6` | `# … ` | Headings 1–6 |
| `/bold` | `**text**` | Bold |
| `/italic` | `*text*` | Italic |
| `/strike` | `~~text~~` | Strikethrough |
| `/quote` | `> ` | Blockquote |
| `/code` | ` ```language\n\n``` ` | Fenced code block |
| `/ul` | `- ` | Bulleted list |
| `/ol` | `1. ` | Numbered list |
| `/task` | `- [ ] ` | Task list item |
| `/link` | `[text](url)` | Hyperlink |
| `/image` | `![alt](url)` | Image |
| `/table` | 2-column table scaffold | Markdown table |
| `/hr` | `---` | Horizontal rule |

Each item renders with its corresponding Lucide icon in the autocomplete dropdown.

---

## `markdown/scrollSync.ts`

**Type:** Bidirectional scroll synchronization controller

Keeps the Markdown editor and the rendered preview pane scrolled to the same logical position in the document.

### Architecture

1. **Anchor map** — Built by reading `[data-line]` attributes injected into the preview HTML by the Markdown renderer. Each anchor maps an editor line fraction to a preview scroll fraction.
2. **Active pane detection** — Pointer and focus events (`pointerenter`, `pointermove`, `pointerdown`, `focusin`, `wheel`, `touchstart`) determine which pane the user is interacting with. Only that pane drives the sync; the other pane is the passive follower.
3. **Lerp animation** — Both panes use a `rAF`-based lerp loop (`factor = 0.25`, `threshold = 0.5px`) for smooth scrolling rather than instant jumps.
4. **Anchor refresh** — A `MutationObserver` on the preview element rebuilds the anchor map 100ms after any DOM change (e.g., re-renders). An `load` capture listener handles image load reflows.

### Key exports

```typescript
// Build normalized anchor map from preview [data-line] elements
buildAnchorMap(editorView: EditorView, previewEl: HTMLElement): ScrollAnchor[]

// Create the sync controller; returns a cleanup/teardown function
createScrollSync(editorView: EditorView, previewEl: HTMLElement): () => void
```

### `ScrollAnchor`

```typescript
interface ScrollAnchor {
    editorFraction: number;   // 0.0 = top, 1.0 = bottom of editor scroll range
    previewFraction: number;  // 0.0 = top, 1.0 = bottom of preview scroll range
}
```

Interpolation between anchors is linear; the edges (before the first anchor and after the last) are linearly extrapolated to ensure the top and bottom of both panes always match.
