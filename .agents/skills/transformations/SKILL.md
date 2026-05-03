---
name: transformations
description: Built-in transformation architecture, shared Rust progress reporting, chunked large-text transport, and CodeMirror apply rules.
---

# Transformation Architecture

This skill documents how built-in transformations work in Grayslate today. Use it when changing transformation behavior, adding new built-ins, altering progress reporting, or moving more heavy text processing from frontend workers into Rust.

## Primary Files

- `src/lib/transformations/actions.ts`
- `src/lib/editor/components/EditorWrapper.svelte`
- `src/lib/ipc.ts`
- `src/lib/editor/core/editorSession.ts`
- `src/lib/editor/core/csvCodeMirror.ts`
- `src-tauri/src/commands/transform.rs`

## End-to-End Flow

1. The frontend chooses a transformation from `actions.ts`.
2. `EditorWrapper.svelte` decides whether the action applies to the current selection or the full document.
3. The frontend sends `execute_transformation` a request containing:
   - `actionId`
   - `text`
   - `requestId` for cancellation correlation
4. Rust runs the transformation inside `spawn_blocking`.
5. Rust sends intermediate `progress` and final `chunk` events through a Tauri `Channel`.
6. Rust returns a small final envelope from the command with:
   - `kind`
   - terminal message metadata
   - `chunkCount` for replace-text results
7. The frontend waits for the expected chunks, builds a CodeMirror `Text` rope, and applies the whole result in one editor transaction.

## Current Architecture Rules

### 1. Control Plane vs Data Plane

- The command response is the **control plane**.
- The Tauri `Channel` is the **data plane**.
- Final metadata must stay in the command response, not the channel.
- Large result text must travel as indexed channel chunks, not as one giant JSON string.

### Why this split exists

- It avoids WebView2 single-message IPC limits for very large text payloads.
- It avoids the race where `invoke()` resolves before the frontend has received all channel events.
- It keeps the transport reusable for future Rust-backed text workers.

### 2. Shared Backend Context

`src-tauri/src/commands/transform.rs` uses `TransformationContext` as the common layer for all built-ins.

New or updated transformations should:

- take `&TransformationContext`
- call `ctx.check_cancelled()` at natural checkpoints
- use `ctx.report_progress(current, total)` when real progress can be reported cheaply
- return through `ctx.run_replace_text(...)` or `ctx.run_show_message(...)`

Do not wire per-action frontend progress behavior. The shared command transport already handles it.

### 3. Progress Model

- Progress events use `{ current, total }`.
- Both values must use the same unit for a given transformation.
- The frontend only converts the ratio to a percent.
- The unit can differ by action:
  - byte progress for streaming/scanning work like JSON minify or CSV parsing
  - row progress when rows are the natural cheap unit

### Progress performance rule

- Progress must come from **natural work checkpoints**.
- Do not add an extra full pass just to count rows or bytes for loader accuracy.
- `TransformationContext.report_progress()` already throttles emissions to roughly 1% steps, so heavy loops can report naturally without spamming IPC.

### 4. Large Text Transport

When a transformation returns replacement text:

- Rust keeps the internal transformation result as a `String`.
- `send_text_chunks()` slices it into `CHUNK_SIZE` pieces.
- Chunk boundaries are moved to valid UTF-8 boundaries.
- Each channel event is `{ type: "chunk", index, text }`.
- The command response returns `chunkCount` instead of the text itself.

### Important transport constraints

- Do not send transformation metadata as a final channel event.
- Do not join backend progress and result delivery into action-specific ad hoc protocols.
- Do not return giant replace-text payloads directly from `invoke()`.
- Keep chunk delivery generic so future Rust workers can reuse the same pattern.

## Frontend Chunk Assembly

`src/lib/ipc.ts` exposes `createChunkedTextAccumulator()`.

The accumulator is responsible for:

- receiving chunk events in any arrival order
- storing chunks by index
- rejecting duplicate or invalid indexes
- rejecting out-of-range data once `chunkCount` is known
- resolving only when every expected chunk has arrived
- resetting state on completion or failure

### Important rule

Do not call `chunks.join("")` for very large results. That can hit browser string-length limits even when the transport itself succeeded.

## CodeMirror Apply Path

`EditorWrapper.svelte` converts ordered chunks into a CodeMirror `Text` rope with `buildCodeMirrorTextFromChunks()`.

This path exists to avoid creating one giant JavaScript string before the editor update.

### Current behavior

- Each chunk is appended into a CodeMirror `Text` structure.
- CRLF split across chunk boundaries is normalized correctly.
- The final result is applied through `dispatchManagedEditorChange()`.
- `insert` accepts `string | Text`, so the editor can take the rope directly.

### Undo/redo contract

- Transformations must apply as a **single CodeMirror transaction**.
- The wrapper uses `separateUndoStep: true`.
- A whole-document transform should still undo in one step.
- Selection transforms also apply in one step to the selected range only.

### Full-document replacement rule

For large whole-document transformations, do not route through a minimal-string diff that first materializes the old and new document text. The current path replaces the document range directly with a `Text` rope to avoid extra peak memory.

## Loader and Cancellation Behavior

### Loader

- Transformations use a grace-period loader.
- There is no fake 0→90 ticker for transformations.
- The loader starts at real 0% and updates only from actual backend progress.
- Final completion is driven by command success/failure, not by a synthetic progress event.

### Cancellation

- Each transform request gets a frontend-generated `requestId`.
- Rust registers that request in `TransformationCancellationRegistry`.
- The loader cancel action calls `cancel_transformation`.
- The frontend suppresses the toast for user-initiated cancellation.

## How To Add or Change a Built-In Transformation

1. Add or update the action definition in `src/lib/transformations/actions.ts`.
2. Add or update the matching `TransformationActionId` in `src-tauri/src/commands/transform.rs`.
3. Implement the backend logic using `TransformationContext`.
4. Pick a natural progress unit only if it can be reported cheaply.
5. Return either:
   - `ReplaceText` for document-changing transforms, or
   - `ShowMessage` for validation/statistics style actions.
6. Let the shared command transport handle progress, chunking, and final delivery.

## Failure Modes To Watch

- Loader jumps or fake progress reappears: inspect `EditorWrapper.svelte` transformation loader logic.
- `invoke()` returns but the editor never updates: inspect the control-plane `chunkCount` and frontend chunk waiter.
- Blank editor on large results: inspect chunk delivery and CodeMirror rope assembly.
- `Invalid string length`: inspect for any accidental `join("")`, `doc.toString()`, or giant-string fallback on the apply path.
- Large transform feels slower after adding progress: inspect whether an extra counting pass was introduced.
- Transport works for small files but not large ones: inspect whether text was accidentally moved back into the command response.

## Safe Change Checklist

- Keep the command response small and authoritative.
- Keep large text on the channel as indexed chunks.
- Preserve UTF-8-safe chunk boundaries.
- Preserve one-transaction undo/redo semantics.
- Keep progress reporting cheap and shared through `TransformationContext`.
- Update both frontend and Rust types together when changing the transport contract.
- Re-run:
  - `pnpm run check`
  - `cargo test --lib -- commands::transform::tests`
