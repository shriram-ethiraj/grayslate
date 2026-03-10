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

## Linux / WebKitGTK gutter rule

Tauri on Linux runs on **WebKitGTK**. CodeMirror's default fixed gutters use sticky positioning. In this repo, that path can fail to repaint line numbers after large scrollbar jumps in long files.

### Required rule

- Keep the normal CodeMirror path on macOS and Windows with `gutters({ fixed: true })`.
- On Linux, use `gutters({ fixed: false })` and attach the Linux-only gutter plugin from `src/lib/editor/extensions/linuxStickyGutter.ts`.

### Why

- WebKitGTK can fail to repaint sticky gutters during vertical scroll.
- The repo avoids that path on Linux by keeping gutters in normal vertical flow and switching modes by scroll axis.
- The Linux plugin uses a fallback `position: relative` plus `translateX(scrollLeft)` path for vertical-safe behavior, and only switches to `position: sticky; left: 0` during pure horizontal scrolling.
- Do not reintroduce compositor-only gutter hacks such as `translateZ(0)` as the primary Linux fix. They were tested and were not reliable in this repo.
- This does **not** materially increase RAM use or document-state cost.

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
