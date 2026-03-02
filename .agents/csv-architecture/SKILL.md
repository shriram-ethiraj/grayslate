---
name: CSV Table Editor Architecture
description: Overview of components, state, and utilities for the high-performance CSV Table Editor.
---

# CSV Table Editor Architecture

This directory contains the components, state, and utilities that power the high-performance CSV Table Editor view in GraySlate.

The CSV Table Editor is built to handle massive CSV files (millions of rows) without freezing the UI, providing an Excel-like editing experience with smooth scrolling, instant cell editing, and reliable undo/redo capabilities.

## High-Level Architecture

The architecture is divided into four main pillars:
1. **Orchestration**: `CsvTableView.svelte`
2. **State & History**: `useCsvEditorState.svelte.ts`, `useCsvHistory.svelte.ts`
3. **Presentation & Virtualization**: `CsvTableBody.svelte`, `CsvTableHeader.svelte`, `useScrollVirtualizer.svelte.ts` (external to this dir)
4. **Web Workers**: `csvParser.worker.ts`, `csvSerializer.worker.ts`

---

## 1. Orchestration (`CsvTableView.svelte`)

This is the entry point and the glue that binds everything together. 

**Key Responsibilities:**
- **Worker Management**: Spawns and terminates the Web Workers for parsing and serializing.
- **State Synchronization**: Acts as the bridge between the raw Markdown/Text string `content` and the `parsed` 2D array state.
- **Debounced Serialization**: Listens for structural changes and triggers debounced serialization (via worker) to update the underlying text content.
- **TanStack Table Instance**: Initializes `@tanstack/svelte-table` purely for **column metadata, resizing, and headers**. It does *not* pass the millions of rows directly to TanStack for performance reasons.
- **Virtualization Init**: Initializes the `useScrollVirtualizer` to manage scroll state.

## 2. State & History Integration

### `useCsvEditorState.svelte.ts`
Handles the immediate, interactive state of the table grid.
- **Focus & Selection**: Tracks which cell is focused (`focusedCell`) and handles Excel-like keyboard navigation (Arrows, Tab, Shift+Tab, Home, End, PageUp, PageDown).
- **Editing**: Tracks the active editing session (`editingCell` and `editValue`). Triggers on `Enter`, `F2`, or standard typing.
- **Mutations**: Exposes methods to apply and reverse structural operations in-place on the `parsed` object arrays without triggering massive reactivity loops, ensuring 60FPS edits.

### `useCsvHistory.svelte.ts`
Provides a structural Undo/Redo stack.
- Instead of saving the entire 50MB CSV string on every keystroke, it saves lightweight `TableOp` events (e.g., `CellEdit`, `RowAdd`, `RowDelete`).
- Capped at `MAX_HISTORY` (200 steps) to preserve memory constraint.
- Calling `undo()` or `redo()` yields the operations to reverse or apply, which the editor state then executes.

## 3. Presentation & Virtualization

To render millions of rows continuously, we avoid standard DOM rendering and TanStack's default body rendering.

- **`CsvTableHeader.svelte`**: Renders the `<thead/>`. Fully sticky. Integrates directly with TanStack's `headerGroups` for accurate sizing and resize handles.
- **`CsvTableBody.svelte`**: Renders the `<tbody/>`. Maps over the `virtualizer.virtualItems` instead of the raw rows. Calculates exact positioning (`translateY`) based on scroll offset.
- **Direct Row Rendering**: In the body, rows are directly indexed (`rawRows[virtualRow.index]`) rendering basic `<td>`s. 

## 4. Web Workers / Data Flow

For large files, synchronous `PapaParse` operations will crash or freeze the main thread. 

### `csvParser.worker.ts`
Used when first entering the Table View, or when the text is modified from the raw Markdown view.
- **Chunking**: Uses PapaParse's `step` function to stream chunks of 500,000 rows back to the main thread incrementally. This allows the UI to show a "Loading X Million rows..." progress indicator instead of hanging.

### `csvSerializer.worker.ts`
Used continuously as edits occur.
- Modifying a cell writes instantly to the Svelte `parsed` array and simultaneously emits a lightweight `TableOp` (Undo/Redo command) to this Serializer worker.
- The worker maintains an **internal, mirrored state** of the entire massive CSV array.
- A debouncer triggers `csvSerializer.worker.ts` with a simple `SERIALIZE` command.
- The worker runs `Papa.unparse` on its internal state in the background and returns the full string, updating the `content` prop.
- **Performance Key**: By sending tiny `TableOp` patches (like `{ row: 5, col: 2, newValue: "X" }`) rather than the entire 2D Svelte array on every keystroke, we completely bypass the browser's "structured clone" algorithm which would otherwise physically copy 50MB of arrays and freeze the main UI thread for hundreds of milliseconds.
- **Note on Unmount**: If the user quickly toggles back to the raw Text View before debouncing finishes, a forced synchronous/immediate `SERIALIZE` worker call is made, delaying UI unmount until serialization completes (`editorState.csv.serializing`).

## Optimization Checklist

If you are modifying this system, prioritize these metrics:
1. **Reactivity**: Keep the `parsed` object as `$state.raw`. Deep Svelte 5 reactivity on millions of arrays will cause immediate out-of-memory crashes.
2. **Column Sizing**: Ensure TanStack Table state is isolated from the row rendering. A column resize should not force a full re-render of the virtualized rows unless absolutely necessary.
3. **Structured Cloning Constraints**: Web Worker `postMessage` is fast for strings but catastrophically slow for deep JS arrays/objects due to structured cloning. Always stream massive data using chunking or by sending patch/diff operations (`TableOp`) instead of passing full arrays across the worker boundary.
4. **Parser Chunk Size**: Keep `csvParser.worker.ts` chunk sizes low (e.g. `50,000` rows maximum). Returning a 500k-row array from a worker instantly halts the main thread during the clone phase, causing visual stuttering in the loading UI.
