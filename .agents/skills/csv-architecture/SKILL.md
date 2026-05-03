---
name: csv-architecture
description: Current implementation reference for CSV table mode, Rust-backed session engine, IPC command surface, virtualization, and thresholded live mirroring into CodeMirror history.
---

# CSV Table Editor Architecture

This skill documents the CSV table implementation that is currently in the repository. Use this file when changing table mode, virtualization, IPC protocols, undo/redo, or the text-mode handoff.

## Primary Files

### Backend (Rust)
- `src-tauri/src/csv.rs` — `CsvSession`, `TableOp`, parsing, serialization, mutation engine, undo/redo (32 unit tests)
- `src-tauri/src/commands/csv.rs` — 9 Tauri command handlers, `CsvSessionRegistry`

### Frontend (Svelte/TypeScript)
- `src/lib/editor/components/EditorWrapper.svelte`
- `src/lib/editor/components/csv/CsvTableView.svelte`
- `src/lib/editor/components/csv/useCsvEditorState.svelte.ts`
- `src/lib/editor/components/csv/useScrollVirtualizer.svelte.ts`
- `src/lib/editor/components/csv/csvTableProtocol.ts`
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
- It owns the Rust session lifecycle (init/dispose), IPC invocations, viewport refreshes, and table metadata shown in the UI.
- It binds `content` to the outer editor wrapper, but table state is driven primarily by Rust IPC responses, not direct DOM editing.
- It keeps `snapshot` and `rowWindow` as raw state objects to avoid deep reactive overhead on large datasets.
- It reports whether the current table session is eligible for live CodeMirror mirroring.

### 4. Rust-Backed Session Engine

- `csv.rs` `CsvSession` is the source of truth for parsed CSV table data while table mode is active.
- The session stores:
	- `headers: Vec<String>`
	- `rows: Vec<Vec<String>>`
	- `delimiter: u8`
	- `errors: Vec<String>`
	- serialized `text` cache (`serialized_text: String`)
	- session-scoped `live_mirror_enabled: bool`
	- `version: u64`
	- `serialized_version: i64` (-1 = dirty)
	- structural `undo_stack` and `redo_stack` (`Vec<Vec<TableOp>>`)
- The session handles parsing, row-window reads, single-cell reads, mutations, undo, redo, and final text flush.
- For non-live-mirrored sessions, the session clears the cached serialized text after each mutation and regenerates it only on `flush_text` to save RAM during large table-editing sessions.

### 5. Session Registry

- `CsvSessionRegistry` in `commands/csv.rs` is managed as Tauri app state, keyed by window label.
- Only one CSV table session per window at a time. A new `csv_initialize` disposes the previous session.
- Follows the `EditorFindState` pattern (per-window `HashMap` behind `Arc<Mutex<>>`).

### 6. Cancellation

- Long-running operations (init parse, flush serialize) use `AtomicBool` for cancellation.
- `csv_cancel` sets the flag; `csv_initialize` checks it every 50,000 rows during parsing.
- Mutations are fast enough in Rust to not need cancellation.

## IPC Command Surface

`commands/csv.rs` defines 9 Tauri commands. `csvTableProtocol.ts` defines the shared TypeScript types.

| Command | Blocking | Returns | Notes |
|---------|----------|---------|-------|
| `csv_initialize` | `spawn_blocking` | `CsvTableSnapshot` via Channel | Parse CSV, store session, report progress |
| `csv_dispose` | sync | `()` | Free session memory |
| `csv_get_rows` | sync | `CsvRowWindow` (JSON) | Slice from in-memory rows |
| `csv_get_cell` | sync | `String` | Direct index lookup |
| `csv_mutate` | sync | `CsvMutationResponse` (JSON) | Apply ops, return snapshot + optional mirror text |
| `csv_undo` | sync | `CsvMutationResponse` (JSON) | Pop undo stack, invert ops |
| `csv_redo` | sync | `CsvMutationResponse` (JSON) | Pop redo stack, apply ops |
| `csv_flush_text` | `spawn_blocking` | `tauri::ipc::Response` (raw bytes) | Serialize if dirty, return via raw byte transport |
| `csv_cancel` | sync | `()` | Cancel in-flight init or flush |

### Transport Choices

- **`csv_initialize`**: Text goes *to* Rust as a string parameter. Progress via `tauri::ipc::Channel<CsvChannelEvent>`. Final snapshot via same channel.
- **`csv_get_rows`**: JSON response (~80KB for 200 rows × 20 cols).
- **`csv_mutate` / `csv_undo` / `csv_redo`**: JSON response with `CsvMutationResponse` including inline `mirrorText: Option<String>`.
- **`csv_flush_text`**: Raw bytes via `tauri::ipc::Response` (bypasses JSON escaping for large text), consumed via `invokeText()`.

### Important Contract Rules

- `CsvTableSnapshot.liveMirrorEnabled` is fixed when table mode is initialized.
- Live mirroring is only enabled for sessions with at most 100,000 data rows at table-entry time.
- When live mirroring is enabled, mutation/undo/redo responses carry `mirrorText` with the latest serialized CSV text for the preserved CodeMirror session.
- `csv_flush_text` returns only the latest serialized CSV text as raw bytes.

## Undo/Redo Model

### Table Mode Undo/Redo

- Table mode undo/redo is structural and Rust-session-owned.
- The session stores arrays of `TableOp` entries rather than full CSV snapshots.
- `undo` pops from `undo_stack`, pushes onto `redo_stack`, inverts the ops, applies them, increments version, and returns updated serialized text (if live-mirror).
- `redo` pops from `redo_stack`, reapplies the ops, pushes back to `undo_stack`, increments version, and returns updated serialized text (if live-mirror).
- History is capped at `MAX_HISTORY = 200` entries.

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
2. `CsvTableView.svelte` calls `invoke("csv_mutate", { mutation, userEvent })`.
3. Rust computes structural ops, applies them, serializes updated CSV text (if live-mirror), increments version, and returns `CsvMutationResponse`.
4. If `mirrorText` is present in the response, `CsvTableView` calls `onMirrorUpdate` inline before snapshot update.
5. `CsvTableView.svelte` updates local snapshot state and refreshes viewport data.
6. The visible row window is refreshed only for the required viewport range.

## Parsing & Serialization

### Parsing (`parse_csv` in `csv.rs`)
- Uses the Rust `csv` crate with streaming record reader.
- Quote-aware delimiter detection (`detect_delimiter`) over candidates `[,\t;|:~]`.
- Greedy empty-line skipping (matches PapaParse `skipEmptyLines: "greedy"`).
- Cancellation check every 50,000 rows via `AtomicBool`.
- Progress reporting via `Channel` at the same interval.

### Serialization (`serialize_csv` in `csv.rs`)
- Uses `csv::WriterBuilder` with the session's delimiter.
- Trailing newline stripped to match PapaParse `unparse` output.
- For live-mirror sessions, serialization happens inline during mutation and the result rides on `CsvMutationResponse.mirrorText`.

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
5. IPC JSON transport for row windows is efficient enough for typical viewport sizes (~200 rows × 20 cols ≈ 80KB).
6. Live mirroring serializes the full CSV text per mirrored step in Rust; do not raise the 100,000-row cutoff casually because the cost scales with total document size.
7. Do not enable live CodeMirror mirroring for table sessions above 100,000 data rows.
8. Do not remove the virtualizer safety caps.
9. Column add/delete on 500K+ rows is O(rows) in Rust but completes in ~5-15ms due to contiguous `Vec::splice` (`memmove` per row). Do not move these back to JS.

## Failure Modes To Watch

- If small CSV sessions collapse into one text-mode undo step, inspect `liveMirrorEnabled`, `mirrorText` in `CsvMutationResponse`, and queue draining in `EditorWrapper.svelte`.
- If large CSV sessions spike memory during table edits, confirm `live_mirror_enabled` stayed false for that Rust session.
- If the last rows become unreachable, inspect the virtualizer's scroll-height mapping.
- If hotkeys stop firing in table mode, inspect DOM focus and element-scoped hotkey registration.
- If switching between text and table causes history drift, inspect the final flush alignment path in `EditorWrapper.svelte`.
- If IPC calls fail with deserialization errors, check `#[serde(rename_all = "camelCase")]` on `CsvMutationRequest` variants with multi-word fields and `#[serde(tag = "type", rename_all = "kebab-case")]` on the enum itself.

## Safe Change Checklist

- Update `csvTableProtocol.ts` together with Rust struct changes (serde names must match).
- Preserve the 100,000-row live mirror cutoff unless the behavior is being intentionally redesigned.
- Keep structural undo/redo inside the Rust session.
- Run `cd src-tauri && cargo test --lib csv::` after Rust changes (32 tests).
- Run `pnpm run check` after any protocol or component change.
- Do not add new JS dependencies for CSV parsing — the Rust `csv` crate handles everything.
