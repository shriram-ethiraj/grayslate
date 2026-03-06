---
name: CSV Table Editor Architecture
description: Overview of components, state, and utilities for the high-performance CSV Table Editor.
---

# CSV Table Editor Architecture

This directory contains the components, state, and utilities that power the high-performance CSV Table Editor view in GraySlate.

The CSV Table Editor is built to handle massive CSV files (millions of rows) without freezing the UI, providing an Excel-like editing experience with smooth scrolling, instant cell editing, and reliable undo/redo capabilities.

## High-Level Architecture

The architecture is divided into four main pillars:
1. **Orchestration**: `EditorWrapper.svelte`, `CsvTableView.svelte`
2. **State & History**: `editorSession.ts`, `useCsvEditorState.svelte.ts`, `useCsvHistory.svelte.ts`
3. **Presentation & Virtualization**: `CsvTableBody.svelte`, `CsvTableHeader.svelte`, `useScrollVirtualizer.svelte.ts` (external to this dir)
4. **Web Workers**: `csvParser.worker.ts`

---

## 1. Orchestration (`EditorWrapper.svelte`, `CsvTableView.svelte`)

These components coordinate mode switches, parsing, and synchronization between the visible CSV table and the preserved CodeMirror document state.

**Key Responsibilities:**
- **On-demand table mounting**: The CSV table only mounts when the user explicitly enters table mode. It does not stay alive in the background.
- **Headless CodeMirror preservation**: `EditorWrapper.svelte` keeps a persistent CodeMirror `EditorState` through `editorSession.ts`, but destroys the heavy `EditorView` while table mode is visible.
- **State Synchronization**: `CsvTableView.svelte` bridges the raw CSV string `content` and the parsed `headers/rows` state.
- **Background reparse**: Text-driven CSV updates are reparsed in the background without tearing down the visible table DOM.
- **TanStack Table Instance**: Initializes `@tanstack/svelte-table` purely for **column metadata, resizing, and headers**. It does *not* pass the millions of rows directly to TanStack for performance reasons.
- **Virtualization Init**: Initializes the `useScrollVirtualizer` to manage scroll state.

## 2. State & History Integration

### `useCsvEditorState.svelte.ts`
Handles the immediate, interactive state of the table grid.
- **Focus & Selection**: Tracks which cell is focused (`focusedCell`) and handles Excel-like keyboard navigation (Arrows, Tab, Shift+Tab, Home, End, PageUp, PageDown).
- **Editing**: Tracks the active editing session (`editingCell` and `editValue`). Triggers on `Enter`, `F2`, or standard typing.
- **Mutations**: Applies structured table operations to parsed data immediately for responsive table-mode UX.
- **Mirroring into CodeMirror**: Every forward table operation, table undo, and table redo computes the resulting CSV text and mirrors it into the preserved CodeMirror session as a normal transaction so text mode can later undo table work.
- **Command ownership**: While table mode is open, undo/redo commands are handled by table history, not by CodeMirror.

### `useCsvHistory.svelte.ts`
Provides a structural Undo/Redo stack.
- Instead of saving the entire 50MB CSV string on every keystroke, it saves lightweight `TableOp` events (e.g., `CellEdit`, `RowAdd`, `RowDelete`).
- Capped at `MAX_HISTORY` (200 steps) to preserve memory constraint.
- Calling `undo()` or `redo()` yields the operations to reverse or apply while table mode is active.

### `editorSession.ts`
Provides the persistent, headless CodeMirror session.
- **Persistent `EditorState`**: Keeps the document, selection, and CodeMirror history alive even after the visible `EditorView` is destroyed.
- **On-demand `EditorView`**: Recreates the real editor DOM only when the user returns to text mode.
- **Mirrored transactions**: Accepts text changes from CSV table operations via `dispatchManagedEditorTextChange()` so CodeMirror history remains useful in text mode.
- **Shared compartments**: Owns theme, language, and word-wrap compartments so the rebuilt `EditorView` resumes from the same state.

## 3. Presentation & Virtualization

To render millions of rows continuously, we avoid standard DOM rendering and TanStack's default body rendering.

- **`CsvTableHeader.svelte`**: Renders the `<thead/>`. Fully sticky. Integrates directly with TanStack's `headerGroups` for accurate sizing and resize handles.
- **`CsvTableBody.svelte`**: Renders the `<tbody/>`. Maps over the `virtualizer.virtualItems` instead of the raw rows. Calculates exact positioning (`translateY`) based on scroll offset.
- **Direct Row Rendering**: In the body, rows are directly indexed (`rawRows[virtualRow.index]`) rendering basic `<td>`s. 

## 4. Web Workers / Data Flow

For large files, synchronous `PapaParse` operations will crash or freeze the main thread. 

### `csvParser.worker.ts`
Used when first entering the Table View, or when the text is modified from the raw Markdown view.
- **Chunking**: Uses PapaParse's `step` function to stream chunks of 50,000 rows back to the main thread incrementally.
- **Request versioning**: Parse messages include a `requestId` so stale parse results can be ignored when multiple text updates arrive quickly.
- **Non-destructive refresh**: Text-driven CSV refreshes keep the current table mounted while a new parse runs, then atomically replace `parsed` when the latest request completes.

There is no serializer worker in the current implementation. Table operations serialize on the main thread and mirror the result into the preserved CodeMirror session.

## Current Sync Rules

- **Text mode visible**: The mounted CodeMirror `EditorView` is authoritative.
- **Table mode visible**: The table is the immediate UI authority, and `useCsvHistory.svelte.ts` owns active undo/redo commands for the table session.
- **Table -> CodeMirror**: Every table change, table undo, and table redo is mirrored into the preserved CodeMirror `EditorState` as a normal transaction.
- **CodeMirror -> Table**: Arbitrary text edits are not translated into semantic table ops. Instead, `CsvTableView.svelte` reparses the current CSV text and swaps in the new parsed result.
- **Mode switches**: The visible CodeMirror `EditorView` is destroyed in table mode and recreated from the preserved `EditorState` when returning to text mode.

## Optimization Checklist

If you are modifying this system, prioritize these metrics:
1. **Reactivity**: Keep the `parsed` object as `$state.raw`. Deep Svelte 5 reactivity on millions of arrays will cause immediate out-of-memory crashes.
2. **Column Sizing**: Ensure TanStack Table state is isolated from the row rendering. A column resize should not force a full re-render of the virtualized rows unless absolutely necessary.
3. **Headless CM state**: Preserve `EditorState` across CSV mode switches, but avoid keeping a hidden `EditorView` mounted for large CSV files.
4. **Parser Chunk Size**: Keep `csvParser.worker.ts` chunk sizes low (e.g. `50,000` rows maximum). Returning a huge row array from a worker instantly halts the main thread during the clone phase.
5. **Refresh stability**: Do not unmount the table for normal text-driven refreshes. Background reparse plus atomic swap is the intended path.
