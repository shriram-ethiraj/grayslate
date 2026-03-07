---
name: Editor Extensions
description: Documentation for custom CodeMirror extensions and editor utilities.
---

# Editor Extensions

This document describes all custom CodeMirror extensions and editor utilities in `src/lib/editor/extensions/`.

> **Note**: For fundamental guidelines on integrating CodeMirror with Svelte and `EditorState` management, see the [CodeMirror Core Integration](../codemirror-core/SKILL.md) skill.

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

1. **Shared coordinate space (`linePercent`)** — Both panes translate through a normalized 0–1 line-progress value rather than mapping pixels directly to each other. This decouples editor geometry (variable line heights) from preview geometry (variable block heights after rendering).
2. **Anchor map** — Built by reading `[data-line]` attributes injected into the preview HTML by `MarkdownPreview.svelte`. Each anchor maps a `linePercent` (source position) to a `previewFraction` (preview scroll position). Sentinel anchors at `{0, 0}` and `{1, 1}` ensure full range coverage. Interpolation is piecewise-linear; edges are extrapolated. Close anchors (< 0.001 apart by `linePercent`) are deduplicated, keeping the one with the higher `previewFraction`.
3. **Active pane detection** — Six interaction events (`pointerenter`, `pointermove`, `pointerdown`, `focusin`, `wheel`, `touchstart`) on both the `.cm-editor` container and the preview element determine which pane owns scrolling. Only the active pane drives sync; the passive pane follows via lerp. Switching panes cancels the follower's pending lerp RAF to avoid fighting the user.
4. **Lerp animation** — Both panes use a rAF-based lerp loop (`LERP_FACTOR = 0.25`, `LERP_THRESHOLD = 0.5px`). The loop terminates when the distance is below threshold and snaps to the exact target.
5. **Anchor refresh** — Anchors are rebuilt via a two-step defer (setTimeout → rAF boundary) to ensure layout is complete before reading `offsetTop` values. Four triggers with separate delays:
   - `MutationObserver` on preview DOM → `MUTATION_REFRESH_DELAY = 90ms`
   - `ResizeObserver` on both preview element **and** editor scroll DOM → `RESIZE_REFRESH_DELAY = 60ms`
   - Image `load`/`error` capture events — `IMAGE_SETTLE_DELAY = 40ms` when all pending images resolve, `IMAGE_REFRESH_DELAY = 140ms` while more images are still loading
6. **Position preservation on refresh** — `applyAnchorRefresh` converts the active pane's current scroll position to `linePercent` using the *old* anchor map before rebuilding. After rebuilding it re-derives the passive pane's scroll target from the new map, so async layout shifts (image loads, re-renders) do not cause visible drift.

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
    linePercent: number;      // 0.0–1.0 normalized source-document progress
    previewFraction: number;  // 0.0–1.0 normalized preview scroll progress
}
```

### `MarkdownPreview.svelte` — renderer integration

The `renderMarkdown` function in `MarkdownPreview.svelte` is what injects the `data-line` attributes consumed by `buildAnchorMap`. It uses `marked`'s `walkTokens` hook to compute source line numbers from token character offsets, then custom renderer extensions add `data-line` to every top-level block element (`heading`, `paragraph`, `code`, `blockquote`, `list`, `table`, `hr`). Only top-level blocks get `data-line`; nested blocks (e.g. a paragraph inside a blockquote) fall through gracefully with no attribute — this is intentional because top-level block density is sufficient for smooth scroll anchoring. Output is sanitized with DOMPurify using `ADD_ATTR: ["data-line"]` to preserve the custom attribute.
