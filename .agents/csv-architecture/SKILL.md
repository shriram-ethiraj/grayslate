---
name: csv-architecture
description: Current implementation reference for CSV table mode, worker data flow, virtualization, and thresholded live mirroring into CodeMirror history.
---

# CSV Table Editor Architecture

This skill documents the CSV table implementation that is currently in the repository. Use this file when changing table mode, virtualization, worker protocols, undo/redo, or the text-mode handoff.

## Primary Files

- `src/lib/editor/components/EditorWrapper.svelte`
- `src/lib/editor/components/csv/CsvTableView.svelte`
- `src/lib/editor/components/csv/useCsvEditorState.svelte.ts`
- `src/lib/editor/components/csv/useScrollVirtualizer.svelte.ts`
- `src/lib/editor/components/csv/csvTableProtocol.ts`
- `src/lib/editor/workers/csvTable.worker.ts`
- `src/lib/editor/core/editorSession.ts`

## Current Architecture

### 1. Mode Ownership

- `EditorWrapper.svelte` owns the mode switch between raw text and CSV table mode.
- Text mode uses CodeMirror through a managed session.
- Table mode mounts `CsvTableView.svelte` on demand.
- While table mode is visible, the heavy CodeMirror `EditorView` is not kept mounted.
- The preserved CodeMirror `EditorState` remains available through `ManagedEditorSession` so text mode can resume without losing document state or history.

### 2. Managed CodeMirror Session

- `editorSession.ts` stores a `ManagedEditorSession` with `state`, optional live `view`, and shared compartments.
- `ensureManagedEditorState()` creates the headless state once and reuses it across remounts.
- `captureManagedEditorView()` stores the latest `EditorState` before the visible editor is destroyed.
- `dispatchManagedEditorChange()` and `dispatchManagedEditorTextChange()` are the only supported ways to push controlled CSV-originated text changes into the preserved session.
- Separate undo steps are created with CodeMirror history isolation annotations, not by rebuilding the document from scratch.

### 3. CSV Table View Responsibilities

- `CsvTableView.svelte` is the orchestration layer for the mounted table.
- It owns the worker lifecycle, request/response bookkeeping, viewport refreshes, and table metadata shown in the UI.
- It binds `content` to the outer editor wrapper, but table state is driven primarily by worker responses, not direct DOM editing.
- It keeps `snapshot` and `rowWindow` as raw state objects to avoid deep reactive overhead on large datasets.
- It reports whether the current table session is eligible for live CodeMirror mirroring.

### 4. Worker-Centered Data Model

- `csvTable.worker.ts` is the source of truth for parsed CSV table data while table mode is active.
- The worker stores:
	- `headers`
	- `rows`
	- `delimiter`
	- `errors`
	- serialized `text` cache
	- session-scoped `liveMirrorEnabled`
	- `version`
	- `serializedVersion`
	- structural `undoStack` and `redoStack`
- The worker is responsible for parsing, row-window reads, single-cell reads, mutations, undo, redo, and final text flush.
- For non-live-mirrored sessions, the worker may drop the cached serialized text after the first mutation and regenerate it only on `flush-text` to save RAM during large table-editing sessions.

## Worker Protocol

`csvTableProtocol.ts` defines the request/response contract.

### Requests

- `initialize`
- `get-rows`
- `get-cell`
- `mutate`
- `undo`
- `redo`
- `flush-text`

### Responses

- `initialize-progress`
- `initialized`
- `rows`
- `cell`
- `mutation-applied`
- `mirror-text-update`
- `flushed-text`
- `error`

### Important Contract Rules

- `CsvTableSnapshot.liveMirrorEnabled` is fixed when table mode is initialized.
- Live mirroring is only enabled for sessions with at most 100,000 data rows at table-entry time.
- When live mirroring is enabled, forward table edits plus table undo/redo emit `mirror-text-update` messages carrying the latest serialized CSV text for the preserved CodeMirror session.
- `flush-text` returns only the latest serialized CSV text and version.

## Undo/Redo Model

### Table Mode Undo/Redo

- Table mode undo/redo is structural and worker-owned.
- The worker stores arrays of `TableOp` entries rather than full CSV snapshots for the active table session.
- `undo` pops from `undoStack`, pushes onto `redoStack`, inverts the ops, applies them, increments version, and returns updated serialized text.
- `redo` pops from `redoStack`, reapplies the ops, pushes back to `undoStack`, increments version, and returns updated serialized text.

### Text Mode Undo After Leaving Table Mode

- For sessions with up to 100,000 data rows, table changes mirror live into the preserved CodeMirror session with isolated undo steps.
- When leaving table mode for those sessions, the wrapper drains pending mirror updates, flushes final text, and only realigns without adding another history entry if needed.
- For larger sessions, live mirroring is disabled for the entire table session.
- When leaving table mode for larger sessions, the wrapper applies one final text update so a single CodeMirror undo returns to the exact pre-table text.

### Reset Rule After Text Changes

- Arbitrary text-mode edits are not converted back into semantic table operations.
- When table mode is entered from modified text, the table is rebuilt from the current text.
- The table-local replay history is reset from that new text baseline.

## Mutation Flow

1. UI action in `useCsvEditorState.svelte.ts` creates a `CsvMutationRequest`.
2. `CsvTableView.svelte` sends the request to the worker.
3. The worker computes structural ops, applies them, serializes updated CSV text, increments version, and returns `mutation-applied`.
4. If `liveMirrorEnabled` is true, the worker also emits a `mirror-text-update` for the preserved CodeMirror session.
5. `CsvTableView.svelte` updates local snapshot state and refreshes viewport data.
6. The visible row window is refreshed only for the required viewport range.

## Virtualization Model

- `useScrollVirtualizer.svelte.ts` handles row virtualization.
- The virtualizer only renders the visible rows plus a bounded overscan window.
- There is a hard cap on virtual items to protect the browser from catastrophic layout behavior.
- The virtualizer must use the effective scroll height of the scroll element when scaled mode is active, otherwise the final rows can become unreachable on browsers with scroll-height caps.
- Do not replace this with naive full-table rendering or unbounded TanStack row rendering.

## TanStack Table Usage

- `@tanstack/svelte-table` is used for column metadata, sizing, and header behavior.
- It is not used as the primary row rendering engine for large CSV datasets.
- Rows in the body are rendered from the virtualized row window, not from a full TanStack row model.

## Performance Rules

1. Keep large table state in `$state.raw` when deep reactivity would be expensive.
2. Do not keep a hidden CodeMirror `EditorView` mounted during table mode.
3. Do not rebuild the full editor state for simple language, wrap, or replay changes.
4. Do not return massive row payloads to the main thread when a narrow row window will do.
5. Let the worker boundary's structured clone handle viewport row transport; do not add an extra deep clone before `postMessage` unless the transport contract changes.
6. Live mirroring currently serializes the full CSV text per mirrored step; do not raise the 100,000-row cutoff casually because the cost scales with total document size.
7. Do not enable live CodeMirror mirroring for table sessions above 100,000 data rows.
8. Do not remove the virtualizer safety caps.

## Failure Modes To Watch

- If small CSV sessions collapse into one text-mode undo step, inspect `liveMirrorEnabled`, worker `mirror-text-update` emissions, and queue draining in `EditorWrapper.svelte`.
- If large CSV sessions spike memory during table edits, confirm `liveMirrorEnabled` stayed false for that table session.
- If the last rows become unreachable, inspect the virtualizer's scroll-height mapping.
- If hotkeys stop firing in table mode, inspect DOM focus and element-scoped hotkey registration.
- If switching between text and table causes history drift, inspect the final flush alignment path in `EditorWrapper.svelte`.

## Safe Change Checklist

- Update `csvTableProtocol.ts` together with worker/frontend changes.
- Preserve the 100,000-row live mirror cutoff unless the behavior is being intentionally redesigned.
- Keep structural undo/redo inside the worker.
- Re-run `pnpm run check` after any protocol or component change.
