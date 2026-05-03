---
name: code-review
description: Reusable review checklist for auditing Grayslate's Svelte 5, TypeScript, CodeMirror, CSV worker, and Tauri backend changes.
---

# Grayslate Code Review Playbook

Use this skill when reviewing changes in this repository. Focus on correctness, security, performance, and architecture fit; do not spend review budget on style-only comments.

## Review Goal

Catch issues that would materially affect:

- security at the Tauri boundary
- editor/session correctness
- CSV table correctness and worker lifecycle
- markdown preview correctness and main-thread performance
- Svelte 5 rune usage and TypeScript safety
- layout safety for flex + virtualizer chains

## Read These First

Before reviewing non-trivial changes, read:

- `agents.md`
- `.agents/svelte-frontend/SKILL.md`
- `.agents/codemirror-core/SKILL.md`
- `.agents/editor-extensions/SKILL.md`
- `.agents/csv-architecture/SKILL.md`
- `.agents/memory-management/SKILL.md`
- `.agents/tauri-backend/SKILL.md`
- `.agents/layout-chain/SKILL.md`

## High-Risk Review Areas

### 1. Tauri trust boundary

Review every command as if the frontend were hostile.

Check for:

- raw paths, command strings, or URLs accepted from the frontend
- missing validation or canonicalization on Rust side
- `unwrap()` / `expect()` in production paths
- blocking filesystem or network work inside async commands
- overly broad capabilities or unsafe config changes in `tauri.conf.json`
- updater changes, especially endpoint/pubkey handling and silent failures

Hot files:

- `src-tauri/src/commands/file.rs`
- `src-tauri/src/commands/update.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/menu/mod.rs`
- `src-tauri/src/window/mod.rs`
- `src-tauri/tauri.conf.json`
- `src-tauri/capabilities/*.json`

### 2. CodeMirror session model

Grayslate preserves `EditorState` even when the live `EditorView` is destroyed.

Check for:

- accidental recreation of editor state where a compartment reconfigure should be used
- Svelte reactivity leaking into CodeMirror state objects
- missing cleanup of DOM listeners, observers, or stored view references
- broken session reuse when switching between text / csv / markdown modes
- unsafe casts hiding prop/session typing mistakes

Hot files:

- `src/lib/editor/components/Editor.svelte`
- `src/lib/editor/components/EditorWrapper.svelte`
- `src/lib/editor/core/editorSession.ts`

### 3. CSV table mode

CSV mode is worker-driven and has special undo/mirroring rules.

Check for:

- worker lifecycle leaks (`Worker`, pending request maps, missing `onerror`)
- race conditions between worker restarts and pending async responses
- incorrect live-mirror behavior around the `100_000` row threshold
- large clone/copy behavior that can blow up memory
- virtualizer math or selection logic that can break under large tables
- violations of layout-chain rules (`min-h-0`, no `height: 100%` in flex children)

Hot files:

- `src/lib/editor/components/csv/CsvTableView.svelte`
- `src/lib/editor/components/csv/useCsvEditorState.svelte.ts`
- `src/lib/editor/components/csv/useScrollVirtualizer.svelte.ts`
- `src/lib/editor/workers/csvTable.worker.ts`
- `src/lib/editor/components/EditorWrapper.svelte`

### 4. Markdown preview

Markdown preview is expensive infrastructure, not a trivial render pass.

Check for:

- heavy parsing/sanitization happening eagerly on every keystroke
- broken scroll sync or active-pane ownership
- missing cleanup of `selectionchange`, scroll, mutation, resize, or image listeners
- any unsanitized HTML path or DOM insertion bypassing `dompurify`
- pointless teardown assignments that do not actually release memory

Hot files:

- `src/lib/editor/components/markdown/MarkdownPreview.svelte`
- `src/lib/editor/components/markdown/scrollSync.ts`
- `src/lib/editor/components/markdown/previewActions.ts`

### 5. Svelte 5 + TypeScript quality

This project is rune-first and strictly typed.

Check for:

- legacy Svelte syntax (`export let`, `$:`, `on:`) unless intentionally isolated to tests/examples
- `$effect` used for pure derivation that should be `$derived`
- untyped or weakly typed `$props()` usage where a props interface would prevent casts
- implicit `any`, `as any`, or repeated casts hiding a better type design
- scattered Tauri `invoke` usage without shared typed wrappers
- string enums that should be `as const` objects when no reverse mapping is needed

## Severity Guidance

Use these buckets:

- **Critical:** security boundary problems, arbitrary file/system access, disabled webview protections, data-loss bugs, crashes in common paths
- **Warning:** incorrect lifecycle, race conditions, blocking I/O, large main-thread work, weak typing at important boundaries
- **Suggestion:** refactors, consolidation of shared IPC/types/constants, non-breaking cleanup

Do not file:

- purely stylistic observations
- comments already enforced by formatter/linter
- tiny naming preferences without correctness impact

## Grayslate-Specific Review Heuristics

### Treat these as suspicious by default

- `setTimeout(..., 0|10|100)` used as a lifecycle barrier
- new `Worker(...)` without `onerror` cleanup
- `invoke(...)` without explicit typing on payload/response intent
- direct filesystem reads/writes from Rust using frontend-provided paths
- `MutationObserver`, `ResizeObserver`, `addEventListener`, or hotkey registration without a cleanup return
- any flex layout changes near CSV/virtualized surfaces that remove `min-h-0`

### Prefer these patterns

- `$derived` for computation, `$effect` only for side effects
- `$props<SomeProps>()` or destructuring with an explicit props type
- Rust `Result<T, E>` with serializable errors instead of panics
- `spawn_blocking` for heavy or blocking backend work
- typed IPC wrappers in shared frontend modules
- explicit teardown paths for workers, observers, and global listeners

## Review Workflow

1. Identify which subsystem changed: frontend UI, editor core, CSV, markdown, backend, or config.
2. Read the relevant skill files above before commenting.
3. Review the changed file in the context of its neighboring lifecycle/transport files, not in isolation.
4. Trace any changed Tauri command from Rust registration to every frontend caller.
5. For UI changes, inspect cleanup paths and flex container chains.
6. For worker or async changes, ask what happens on restart, unmount, cancellation, and error.
7. Prefer one high-signal comment per root cause, with all affected files/lines grouped together.

## Validation Expectations

When possible after substantive changes:

- `pnpm run check`
- `pnpm run tauri build`

If validation is blocked by environment issues, note that explicitly in the review.
