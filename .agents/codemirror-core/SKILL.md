---
name: codemirror-core
description: Rules for integrating CodeMirror 6 with Svelte, managing performance, and writing scalable extensions.
---

# CodeMirror Core Integration

This document outlines the core integration patterns for CodeMirror 6 inside Grayslate. For specific editor extensions (like hover tooltips or autocomplete), see `.agents/editor-extensions/SKILL.md`.

## Integration Guidelines

- **Use `@codemirror/state` and `@codemirror/view` correctly.**
- **Conceptual Separation:** Keep the CodeMirror `EditorState` conceptually separated from Svelte's `$state` unless explicitly synchronizing document content. Avoid deep reactivity loops between the two.
- **Clean Transactions:** Dispatch `Transaction` objects cleanly for editor updates rather than violently replacing the document text.
- **Performance First:** When building extensions (Fold widgets, Tooltips, Inlay Hints), **never** write unbounded `while` loops that traverse the Lezer tree (e.g. counting every child of a JSON Array). Cap iterations aggressively (e.g. `MAX_SCAN_CHILDREN = 100`) to prevent the main thread from freezing when users paste gigabyte-sized files.
- **Experimental Extensions:** Unused or WIP CodeMirror extensions (like `stickyScroll`) are kept in `src/lib/editor/extensions/experimental/`. Do not delete these files, but do not import them into the main `languageExtensions.ts` config unless specifically requested.

## Gutter rule

Use CodeMirror's native fixed gutters across all supported platforms.

### Required rule

- Keep `gutters()` in the main editor session.
- Do not add a Linux-only gutter plugin or platform branch unless there is a verified regression and a measured need for it.

`gutters()` already uses CodeMirror's native fixed gutter behavior by default, so there is no need to pass `fixed: true` explicitly.

### Why

- The editor should stay on the simplest supported CodeMirror gutter path by default.
- Platform-specific gutter behavior increases maintenance cost and makes editor rendering harder to reason about.
- If Linux / WebKitGTK gutter rendering is revisited later, treat it as a fresh investigation rather than restoring the removed workaround by habit.

## Editor-surface CSS performance guidance

For the main CodeMirror editor surface in this repo:

- **Safe to keep:** `height: 100%` on `.cm-editor`
- **Safe to keep:** `overscroll-behavior: none` on `.cm-scroller`
- **Avoid changing casually:** `contain: strict` on `.cm-editor`
- **Avoid changing casually:** `will-change: transform` on `.cm-content`

### Rationale

- `overscroll-behavior: none` is harmless here and not related to the gutter repaint bug.
- `contain: strict` and `will-change: transform` are not CodeMirror defaults.
- Treat them as profiling-sensitive knobs because they can interact badly with WebKitGTK repaint/compositing behavior and make editor rendering harder to reason about.
- If performance tuning is revisited later, do it from a measured profile and test Linux explicitly with large files and scrollbar dragging.

## Find and replace worker flow

The in-editor find/replace UI is a custom Svelte panel, not CodeMirror's built-in search panel.

### Current split of responsibilities

- `src/lib/editor/components/FindReplace.svelte` owns the popup UI, local input state, debounce timing, and flush-before-navigate / replace behavior.
- `src/lib/editor/core/actions.ts` owns the main-thread CodeMirror integration for `setSearchQuery`, highlights, next/previous navigation, replace actions, and worker request dispatch.
- `src/lib/editor/workers/findStats.worker.ts` owns the expensive full-document scan for `matchCount` and `currentMatch`.
- `src/lib/editor/workers/findStatsProtocol.ts` is the shared message contract and should be updated together with worker/frontend changes.

### Important architecture rule

- Keep CodeMirror search state on the main thread.
- Keep expensive counting and current-match computation off the main thread in the worker.
- Do not move `findNext`, `findPrevious`, `replaceNext`, or `replaceAll` into the worker because they need the live `EditorView`.

### Important update-flow rule

`setSearchQuery` by itself does not count as a document change or selection change.

That means a pure search-query dispatch will not reliably flow through the managed-session `onViewUpdate` path that is used for document/selection synchronization. In this repo, `editorSetSearchQuery()` must explicitly call `updateSearchStats(view)` after dispatching the new `SearchQuery`, otherwise the worker can miss query changes and the visible count can go stale.

### UX behavior to preserve

- Opening find from a selection should seed the find field from the current selection.
- Closing find should clear the seeded or previously typed find text.
- While a new worker result is in flight, keep showing the previous resolved count instead of replacing it with a flashing loader or placeholder text.
- Navigation and replace actions should flush any pending debounced query update first so the action uses the text currently visible in the input.

### Worker lifecycle guidance

- Clear worker state when the panel closes or when the search becomes empty.
- Ignore stale worker responses by request ID so older scans cannot overwrite newer results.
- Be careful with `doc.toString()` on large documents: the worker removes the scan cost from the UI thread, but main-thread serialization is still real work and should not be duplicated unnecessarily.
