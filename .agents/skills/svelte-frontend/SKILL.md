---
name: svelte-frontend
description: Rules for writing Svelte 5, strictly typed TypeScript, and frontend coordination patterns in Grayslate.
---

# Svelte Frontend Guidelines

## Core Principles

- **Embrace Svelte 5 Runes:** Exclusively use modern Svelte 5 signals (Runes).
  - Use `$state()` for reactive state.
  - Use `$derived()` for computed values.
  - Use `$effect()` for side effects.
  - Use `$props()` instead of `export let` for component inputs.
  - **Avoid legacy Svelte 4 features** (`$:`, legacy slot architecture). Opt for Svelte 5 `{#snippet}` when handling template injection.
- **Strong Typing:** Do not use `any`. Define strictly typed interfaces and types for all component props, Tauri IPC payloads, and CodeMirror extensions.
- **Vite Native:** Keep assets optimized. Import static assets cleanly and let Vite handle caching and bundling.
- **Tooltip Preference:** Prefer native `title` attributes for simple hover labels such as full paths. Do not add custom tooltip components unless the interaction clearly requires richer content or behavior.

## Shared Sidebar Coordination

For cross-component coordination between the editor, dialogs, and the library sidebar, prefer the shared state surface in `src/lib/state/librarySidebar.svelte.ts` over custom event chains.

Current coordination hooks:

- `pendingOpenFile`
- `requestActivateSearch`
- `handleLibraryMutation` / `reportLibraryMutation(...)`

Use these for sidebar/editor/dialog interaction where the components do not have a direct parent-child data flow.

## Sidebar Reorder Suppression Policy

The library sidebar has an explicit UX invariant:

- when a user opens a file from the sidebar, the visible recent-files list must not jump under the cursor

Implementation lives in `src/lib/components/app-sidebar.svelte`.

Key pieces:

- `suppressReorder`
- `lastSidebarOpenedPath`
- `createLibraryRefreshCoordinator(...)`

Rules:

- opening a file from the sidebar activates suppression
- while suppressed, background refreshes are deferred
- successful structural mutations release suppression and refresh immediately
- pure filter-tab changes do not clear suppression
- suppression clears on explicit user actions like sort change or manual refresh
- sidebar close/reopen performs an invisible quiet refresh so reopen shows fresh, already-sorted data

Do not reintroduce eager reordering or a second staged list unless you have verified the UX impact.

## Structural Mutation Sync

Dialogs and editor flows report semantic mutations after successful operations:

1. report the operation through `reportLibraryMutation(...)`
2. let the sidebar own active-dataset refresh, suppression, tab, and reveal policy

Do not add operation-specific refresh calls outside the coordinator.

## Linux / WebKitGTK Rendering Notes

Tauri on Linux uses **WebKitGTK**, and some rounded UI surfaces render differently than on macOS or Windows. In particular, WebKitGTK is prone to border shimmer, broken corners, or twitchy seams when the same rounded element combines:

- `border`
- `border-radius`
- `overflow-hidden`
- nested clipping wrappers
- animated hover / selected backgrounds that paint to the edge

This has already affected recent-file cards, dialog shells, and command/dialog compositions in this repo.

### Known-safe pattern for rounded shells

When a rounded surface needs clipped inner content:

1. **Prefer an outer shell for the visual outline**.
2. **Clip on an inner wrapper**, not on the outer bordered shell.
3. On Linux-sensitive surfaces, prefer an **inset ring** over a physical border when the border itself flickers.
4. If the inner content is full-bleed, leave a **1px reveal** so the outer outline stays visible.

Typical structure:

```svelte
<Shell class="rounded-lg ring-1 ring-inset ring-border">
  <div class="m-px overflow-hidden rounded-[calc(var(--radius-lg)-1px)]">
    <!-- clipped content here -->
  </div>
</Shell>
```

### Do / Don't

- **Do** use `m-px` on an inner wrapper when a full-bleed child would otherwise visually cover the outer ring.
- **Do** keep selected-state styles stronger than hover-state styles; hover must not override active styling.
- **Do** test dialogs, menus, cards, and popovers on Linux after changing border/radius/overflow behavior.
- **Don't** put `overflow-hidden` on the same rounded element that owns the physical `border` unless you have verified Linux rendering.
- **Don't** assume a fix that looks correct on macOS/Windows will behave the same on Linux.

### When to suspect this issue

If a Linux-only bug report mentions any of the following, inspect rounded shell composition first:

- border looks broken on one side
- border flickers while moving the mouse
- corners shimmer during hover or focus
- one pane of a split dialog shows the outline while the other appears flush
- popup/dialog outline disappears when content becomes full-bleed

In those cases, inspect the nearest shared primitive first (`Dialog.Content`, item/card shells, popover/select/menu content), then inspect app-level wrappers that add `overflow-hidden` or full-bleed backgrounds.
