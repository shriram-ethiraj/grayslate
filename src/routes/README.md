# Layout Chain Documentation

> **⚠️ Critical reading for any agent or developer modifying layout, sidebar, or the CSV table view.**
>
> Incorrect CSS changes to this chain have caused CPU/memory spikes that crash the application.  
> The root cause each time: breaking `min-h-0` out of the flex-column chain, which feeds a garbage `containerHeight` into the CSV virtualizer and triggers millions of DOM nodes being created.

---

## The Full DOM Flex Chain

The app has a deeply nested **flex-column** chain from `<html>` all the way to the scroll container inside the CSV table. Every link in this chain must be honoured or height containment breaks.

```
<html>  (h-dvh, overflow-hidden)
  <body>  (flex flex-col, h-dvh, overflow-hidden)
    <div class="flex h-screen flex-col overflow-hidden">    ← root wrapper in +layout.svelte
      <Titlebar />                                          ← shrink-0, fixed height
      <div class="relative flex-1 overflow-hidden">        ← flex-1: takes remaining vertical space
        <Sidebar.Provider class="h-full min-h-0">          ← h-full + min-h-0: must both be present
          <ResizablePaneGroup direction="horizontal">       ← flex h-full w-full (paneforge)
            <ResizablePane id="sidebar">                   ← overflow:hidden inline (paneforge, load-bearing)
              ...AppSidebar...
            </ResizablePane>
            <ResizablePane id="content" class="flex flex-col">  ← overflow:hidden inline (paneforge)
              <Sidebar.Inset class="min-w-0 min-h-0 overflow-hidden">  ← ⚠️ ALL THREE required
                <header class="shrink-0 h-12 ...">         ← fixed-height toolbar
                <div class="flex flex-1 flex-col min-h-0 min-w-0">  ← flex-1 + min-h-0
                  <!-- +page.svelte renders children here -->
                  <EditorWrapper>
                    <div class="flex flex-1 flex-col min-h-0 min-w-0">
                      <div class="flex flex-1 min-h-0 min-w-0 relative">
                        <!-- CSV active: -->
                        <CsvTableView>
                          <div class="csv-table-wrapper">   ← flex:1; min-height:0  ⚠️ NOT height:100%
                            <div class="csv-table-container">  ← flex:1; overflow:auto ← THE SCROLL ELEMENT
                              <div style="height:{virtualizer.totalSize}px">  ← up to 30,000,000px tall
```

---

## Why `min-h-0` Is Non-Negotiable

Flex items in a **flex-column** container default to `min-height: auto`. This means the browser computes the item's minimum height as its content's intrinsic height. If any item's content can grow arbitrarily (as the virtualizer's inner content div can — up to 30,000,000 px), the entire chain above it expands to match because no container has told it to stop.

`min-h-0` overrides this to `min-height: 0`, allowing the flex item to shrink to whatever space its parent allocates, regardless of how tall its content is.

**Golden rule:** Any element in a flex-column chain that has `flex: 1` (or `flex-grow: 1`) **and** contains vertically-growing content below it **must** also have `min-h-0`.

---

## Why `height: 100%` Is Forbidden Inside This Chain

`height: 100%` only resolves to a definite value when the parent has a **definite, explicit height** (a fixed pixel value or is itself sized by a definite ancestor). In a flex chain, flex items do not have a "definite" height in the CSS spec sense unless the parent is `display: block` with a fixed height.

Inside a flex-column chain, `height: 100%` typically resolves to the **content's intrinsic height**, not the parent's allocated space. This causes the element to expand to its content size instead of capping at the viewport.

**Always use `flex: 1; min-height: 0`** (Tailwind: `flex-1 min-h-0`) in place of `height: 100%` anywhere in this layout chain.

---

## Node-by-Node Rules

### `<div class="relative flex-1 overflow-hidden">` (in +layout.svelte)
- `flex-1`: takes all vertical space after the `<Titlebar>`.
- `overflow-hidden`: prevents any child from visually escaping the viewport. **Do not remove.**

### `<Sidebar.Provider class="h-full min-h-0">`
- `h-full`: fills the `flex-1` parent.
- `min-h-0`: required because `Sidebar.Provider` renders a `<div>` with `flex` that would otherwise hold `min-height: auto`. Without it, the paneforge group inside can report an inflated height.

### `<ResizablePane>` (both sidebar and content)
- Paneforge applies `overflow: hidden` **as an inline style** programmatically via its `getPaneStyle` utility. This is load-bearing — it prevents the pane's content from overflowing and breaking the sizing contract.
- **Never override `overflow` on a `ResizablePane`.** If you need overflow inside a pane, add a wrapper div inside it.

### `<Sidebar.Inset class="min-w-0 min-h-0 overflow-hidden">`
- The shadcn default for `sidebar-inset.svelte` is `bg-background relative flex w-full flex-1 flex-col`. It has **no `min-h-0`** and **no `overflow-hidden`** by default.
- `flex-1` means it grows inside the `ResizablePane` (which is already a flex-column via `class="flex flex-col"`).
- Without `min-h-0`, `Sidebar.Inset` refuses to shrink below its content, propagating content heights upward.
- Without `overflow-hidden`, overflow from children (especially during virtualizer initialisation) can escape and cause layout reflow loops.
- **All three classes (`min-w-0 min-h-0 overflow-hidden`) must always be present on `Sidebar.Inset`.**

### `.csv-table-wrapper` (in CsvTableView.svelte)
```css
.csv-table-wrapper {
    display: flex;
    flex-direction: column;
    flex: 1;          /* ← grows to fill EditorWrapper's flex container */
    min-height: 0;    /* ← MUST be here — see above */
    width: 100%;
    overflow: hidden; /* ← clips virtualizer overflow during scroll */
}
```
- **Do not change `flex: 1; min-height: 0` back to `height: 100%`.** This was the direct cause of the CPU crash that was fixed. `height: 100%` inside a flex item without a definite parent height expands to content size.

### `.csv-table-container` (in CsvTableView.svelte)
```css
.csv-table-container {
    flex: 1;
    overflow: auto;         /* ← the actual scroll element read by the virtualizer */
    overscroll-behavior: none;
    position: relative;
}
```
- This is the element bound to `tableContainerRef` and passed to `useScrollVirtualizer` as `getScrollElement`.
- The virtualizer's `ResizeObserver` watches this element's `contentRect.height`.
- If the chain above is broken and this element's height is not constrained, the virtualizer receives a gigantic `containerHeight`.

---

## The Virtualizer Safety Caps

Even with a correct CSS chain, `useScrollVirtualizer.svelte.ts` has two defensive safety nets. **Do not remove them.**

### 1. `containerHeight` clamp
```typescript
containerHeight = Math.min(h, window.innerHeight * 3);
```
If a layout change causes the scroll element's reported height to exceed `3 × window.innerHeight`, it is clamped. This prevents a developer mistake from instantly crashing the app — the table will just show a subset of rows until the CSS is fixed.

### 2. `MAX_VIRTUAL_ITEMS = 200`
```typescript
const endIdx = Math.min(rawEndIdx, startIdx + MAX_VIRTUAL_ITEMS - 1);
```
No matter what `containerHeight` or `overscan` compute, the virtualizer will never create more than 200 DOM rows at once. At 32 px per row, 200 rows = 6400 px rendered, which is more than any real monitor height. This cap costs nothing in normal use and is a hard ceiling against runaway DOM creation.

---

## The Draggable Sidebar Architecture

The sidebar uses a **hybrid** of shadcn's `Sidebar.*` components (for state, keyboard shortcut, and trigger button) and **paneforge** (for actual draggable sizing). The standard shadcn `Sidebar.Sidebar` component is intentionally **not used** because it manages sizing via CSS variables and cannot be made draggable.

### How it works:
1. `Sidebar.Provider` provides open/close state and handles `Ctrl+B`.
2. `ResizablePane id="sidebar"` handles actual pixel sizing.
3. `Sidebar.Trigger` button is placed in the toolbar and calls `Sidebar.Provider`'s toggle, which is wired to `handleOpenChange()`.
4. `handleOpenChange()` calls `sidebarPane?.collapse()` or `sidebarPane?.expand()` **imperatively** after enabling `paneCollapsible = true` so paneforge will honour the call.

### The `paneCollapsible` toggle:
- When the pane is **open and the user drags**: `paneCollapsible = false`. This prevents the pane from snapping to collapsed if the user accidentally drags past `minSize`.
- When the pane is **collapsed or the Trigger is pressed**: `paneCollapsible = true`. This re-enables the ability to expand/collapse programmatically and to drag from the collapsed state.

### The `animating` flag:
- The CSS transition (`transition-[flex-grow] duration-200`) is only applied during **programmatic toggle** (button click / Ctrl+B). It is removed immediately after the animation completes (after 210ms timeout).
- During drag, no animation class is applied, giving immediate visual feedback.
- This prevents Svelte from re-rendering with the transition class while the user is dragging, which would cause laggy pane resizing.

### Why `--sidebar-width: 100%` is set on the sidebar pane:
The `Sidebar.Provider` CSS context uses `--sidebar-width` to define the sidebar's intended width (default `16rem`). Because paneforge controls actual sizing, the sidebar content div is set to `--sidebar-width: 100%` so `AppSidebar` fills whatever width paneforge allocates, rather than being capped at `16rem`.

---

## What to Check Before Any Layout Change

Before modifying any of the following, re-read this document:

| File | What to check |
|------|---------------|
| `src/routes/+layout.svelte` | Every flex item in the vertical chain has `min-h-0`. `Sidebar.Inset` has `min-h-0 overflow-hidden`. |
| `src/lib/components/ui/sidebar/sidebar-inset.svelte` | The shadcn component does not add `min-h-0` by default. Always pass it via `class` prop. |
| `src/lib/editor/components/csv/CsvTableView.svelte` | `.csv-table-wrapper` uses `flex: 1; min-height: 0`, NOT `height: 100%`. |
| `src/lib/editor/components/csv/useScrollVirtualizer.svelte.ts` | `MAX_VIRTUAL_ITEMS` and `containerHeight` clamp are present and not removed. |
| `src/lib/editor/components/EditorWrapper.svelte` | Outer `<div>` has `flex-1 flex-col min-h-0 min-w-0`. Inner content pane has `min-h-0`. |

---

## History: What Broke and Why

**Incident:** After adding draggable sidebar (paneforge integration), switching to CSV table view caused CPU and memory to spike to 100% and crash the app.

**Root cause chain:**
1. `Sidebar.Inset` was missing `min-h-0` and `overflow-hidden` after the layout rewrite.
2. `.csv-table-wrapper` in `CsvTableView.svelte` used `height: 100%` (incorrect inside a flex chain).
3. The virtualizer's scroll container grew to match its inner content (30,000,000 px).
4. `ResizeObserver` reported `containerHeight ≈ 30,000,000`.
5. `visibleCount = ceil(30,000,000 / 32) ≈ 937,500`.
6. A virtual item array of ~937,500 entries was created, each causing a DOM row to be inserted.
7. The browser's DOM mutation cost exploded → CPU spike → OOM → crash.

**Fixes applied:**
- Added `min-h-0 overflow-hidden` to `Sidebar.Inset` in `+layout.svelte`.
- Changed `.csv-table-wrapper` to `flex: 1; min-height: 0`.
- Added `containerHeight` clamp (`window.innerHeight × 3`) in the virtualizer.
- Added `MAX_VIRTUAL_ITEMS = 200` hard cap in the virtualizer.
