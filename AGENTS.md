# Grayslate — Agent Guidelines

Reference for any AI coding agent (Codex, Codex, opencode, etc.) working on this repository.

## Project Overview

**Grayslate** is a lightweight, cross-platform developer scratchpad. It features built-in functions for handling data formats like JSON and CSV, and allows users to add their own custom functions. Notes are auto-saved, can be custom-named by the user, and are automatically synced to Git.

## Tech Stack & Constraints

- **Desktop Framework:** Tauri v2
- **Backend Languages:** Rust
- **Frontend Framework:** Svelte 5 (SvelteKit with Static Adapter)
- **Frontend Language:** TypeScript
- **Editor Engine:** CodeMirror 6
- **Bundler:** Vite
- **Package Manager:** pnpm

---

## Where To Look

Keep this file compact. Detailed implementation notes belong in the skill files.

- Frontend patterns: `.agents/skills/svelte-frontend/SKILL.md`
- Code review playbook: `.agents/skills/code-review/SKILL.md`
- CodeMirror session model: `.agents/skills/codemirror-core/SKILL.md`
- Editor extension patterns: `.agents/skills/editor-extensions/SKILL.md`
- CSV table architecture: `.agents/skills/csv-architecture/SKILL.md`
- Naming and SQL naming architecture: `.agents/skills/naming-architecture/SKILL.md`
- Transformation architecture: `.agents/skills/transformations/SKILL.md`
- Sidebar search architecture: `.agents/skills/search-architecture/SKILL.md`
- Hotkeys: `.agents/skills/tanstack-hotkeys/SKILL.md`
- Memory reclamation: `.agents/skills/memory-management/SKILL.md`
- Tauri backend: `.agents/skills/tauri-backend/SKILL.md`
- Layout safety rules: `.agents/skills/layout-chain/SKILL.md`
- Typography & spacing: `.agents/skills/typography/SKILL.md`

Codex also sees these skills via `.Codex/skills` (symlinked to `.agents/skills`) and can invoke them automatically or with `/skill-name`.

Language detection and naming logic live in workspace crates, not under `src-tauri/src/`: `crates/grayslate-langdetect/` and `crates/grayslate-langnaming/`. `src-tauri/src/detection.rs` and `src-tauri/src/naming.rs` are thin re-export shims — see the naming-architecture skill above for the full layout.

## Current High-Level Reality

- File open flows through `EditorWrapper.svelte` into Rust `read_file_content`, with a current 200 MB backend-enforced limit.
- CodeMirror document state is preserved in a managed session even when the live editor view is destroyed.
- Find/replace uses a custom Svelte panel; CodeMirror still owns highlights and navigation on the main thread, while match-count/current-match stats are computed in Rust (`src-tauri/src/findstats.rs`, via `invoke("editor_find_scan", ...)`) so large-document scans don't block typing. There is no JS Web Worker in this flow.
- Built-in transformations use a shared Rust progress/cancellation context plus a chunked large-text transport; the frontend assembles chunked results into a CodeMirror `Text` rope and applies them as one undoable transaction.
- CSV table mode is Rust-backed and mounted on demand.
- CSV table edits mirror live into the preserved CodeMirror session only for sessions that start at or below 100,000 data rows; larger sessions skip live mirroring and return to text mode as a single undo step back to the pre-table state.
- Markdown preview uses Rust-side `pulldown-cmark` plus `ammonia`, with sanitized HTML returned over raw-byte IPC and custom scroll-sync hooks.
- Loader and memory-reclamation behavior are shared infrastructure, not CSV-specific logic.

---

## Architecture & Coding Standards

### 1. Frontend (Svelte 5 & TypeScript)

**> To know more about this topic, YOU MUST READ the `.agents/skills/svelte-frontend/SKILL.md` file.**

- **Embrace Svelte 5 Runes:** Exclusively use modern Svelte 5 signals (`$state`, `$derived`, `$effect`, `$props`). Avoid Svelte 4 legacy features.
- **Strong Typing:** Do not use `any`. Use strict TypeScript interfaces.
- **Vite Native:** Let Vite handle assets and bundling.
- **Memory Efficiency:** Aggressively clean up memory on component unmount (`onDestroy`). Explicitly nullify `$state` variables, large arrays, objects, and external DOM references (e.g., CodeMirror views) when a component is destroyed, especially in expensive views (Diff, CSV, Markdown).

### 2. Editor Integration (CodeMirror 6)

**> To know more about core integration, YOU MUST READ the `.agents/skills/codemirror-core/SKILL.md` file.**
**> To know more about custom extensions, YOU MUST READ the `.agents/skills/editor-extensions/SKILL.md` file.**

- Keep `EditorState` separate from Svelte's `$state` to avoid reactivity loops.
- Perform updates via `Transaction`s.
- Preserve document state in a managed session even when the live `EditorView` is unmounted.
- Use compartments for language/theme/word-wrap reconfiguration instead of rebuilding the editor state unnecessarily.
- **Performance:** Cap Lezer tree traversals to avoid freezing the main thread.

### 3. Desktop / Backend (Tauri v2 & Rust)

**> To know more about backend rules, YOU MUST READ the `.agents/skills/tauri-backend/SKILL.md` file.**

- Ensure usage of Tauri **v2** APIs.
- Use `Result<T, E>` and `serde::Serialize` for returning Rust errors to Svelte.
- Use async functions for I/O to avoid blocking. Validate all payloads.

### 4. Layout & CSS — Critical Rules

**> To know more about layout issues and fixes, YOU MUST READ the `.agents/skills/layout-chain/SKILL.md` file.**

> **Breaking these rules causes catastrophic virtualizer failures (CPU/memory spikes, app crash).**

- **Never use `height: 100%` inside flex children.** Use `flex-1 min-h-0`.
- **Every flex-column container and its flex children must have `min-h-0`.**
- **`Sidebar.Inset` must always have `min-h-0 overflow-hidden`**.

### 5. Application Features & Core Libraries

**> To know more about the CSV architecture and virtualizer, YOU MUST READ the `.agents/skills/csv-architecture/SKILL.md` file.**
**> To know more about built-in transformations and large-text transport, YOU MUST READ the `.agents/skills/transformations/SKILL.md` file.**
**> To know more about keyboard shortcut management (hotkeys), YOU MUST READ the `.agents/skills/tanstack-hotkeys/SKILL.md` file.**
**> To know more about memory management and GC pressure, YOU MUST READ the `.agents/skills/memory-management/SKILL.md` file.**

- **Language Detection:** Uses a Rust-side family-first pipeline in the `crates/grayslate-langdetect/` crate (re-exported through `src-tauri/src/detection.rs`): extension / filename → shebang → strong structural probes → content family classification → family-gated scoring → rival disambiguation / superset tiebreak, abstaining when no confident match is found. The FE retains only thin sync UI maps (`languageExtensions.ts`, `languageIconMap.ts`) while all content-based detection still goes through IPC.
- **Naming:** Uses a per-language `NamingDefinition` registry in the `crates/grayslate-langnaming/` crate (re-exported through `src-tauri/src/naming.rs`) for canonical save extensions and extractor routing. `suggest_stem_auto()` returns the effective detected language, and `email` / `prompt` are first-class naming kinds that force `-email` / `-prompt` suffixes.
- **Library Sidebar:** Recent-files refresh is backend-driven via `files://recent-updated`, emitted after read/save/rename/delete/duplicate flows. The sidebar suppresses reordering after sidebar-initiated opens and uses quiet refresh + rename-path tracking so the visible list does not jump under the cursor. Sidebar search uses cooperative cancellation with `cancel_sidebar_search` IPC for immediate backend stop on keystroke/close/teardown; the debounce effect uses `isSearchMode` (not `normalizedQuery`) to avoid per-keystroke reactive leaks.
- **Memory Management:** Uses a Rust `sysinfo` integration and a frontend "GC Pressure" trick to reclaim heap after expensive editor teardown, especially after file switches.
- **CSV Table View:** Uses a custom scroll virtualizer with hard safety caps; see the CSV skill for the current details.
- **CSV Mode Architecture:** CSV table mode mounts on demand, performs parsing and mutations in a Rust `CsvSession` backend via Tauri IPC, and only live-mirrors text undo history for sessions that start at or below 100,000 data rows.
- **Transformations:** Built-in transformations share a Rust-side progress/cancellation layer and use chunked text delivery plus CodeMirror rope assembly for large results.
- **Find / Replace:** Uses a custom popup wired to CodeMirror search state; heavy match counting is Rust-backed via Tauri IPC (`editor_find_scan` / `editor_find_selection` / `cancel_editor_find`), but live query/highlight/navigation stays on the main thread.
- **Markdown Preview:** Parsed and sanitized in Rust via `pulldown-cmark` and `ammonia`, with custom bi-directional scroll synchronization. Saved files resolve relative images through a bounded image-only IPC command; external links are opened through the system browser rather than navigating the app webview.
- **Hotkeys:** Use `@tanstack/hotkeys` through the shared helpers in `src/lib/hotkeys.ts`; table-specific shortcuts should remain element-scoped.
- **File Loading:** File reads are validated in Rust and currently allow files up to 200 MB.

---

## Agent Instructions

When generating code or proposing architectural changes, adhere to the following rules:

1.  **Reflect the Stack:** Always provide solutions in standard Rust (Edition 2021) and Svelte 5.
2.  **No Hallucinations:** Check Tauri 2.0 and CodeMirror 6 documentation bounds. These APIs have changed significantly from their previous major versions; double-check the syntax before writing out plugin configs or IPC logic.
3.  **Readability over Cleverness:** Write code that is maintainable. Comment complex regex, complex Rust lifetimes, and custom CodeMirror state fields heavily.
4.  **Security First:** When using Tauri APIs, assume the frontend is untrusted. Validate all payloads on the Rust side before execution, particularly if writing files or running binaries.
5.  **Conciseness:** Provide the exact code block needed to fix the issue. Avoid unnecessary pleasantries or overly long explanations unless asked to explain the architectural choice.

## Commits & Workflow

> ### CRITICAL: NEVER AUTO-COMMIT
> **Do not run `git commit` under ANY circumstances.** Not as a "helpful" final step, not in autopilot mode, not to "save" progress. This applies even if you have a commit message template or Co-authored-by trailer available. Stage changes with `git add` if needed, but STOP THERE. The developer commits manually after reviewing staged changes.

- Keep `.gitignore` respected (e.g., `node_modules`, `target`, `.svelte-kit`).
- Verify code works with `pnpm run check` (runs `svelte-check`), `cargo test --manifest-path src-tauri/Cargo.toml` (workspace tests, including `crates/grayslate-langdetect` and `crates/grayslate-langnaming`, which have substantial unit coverage), and compiles with `pnpm run tauri build`.
- The frontend (Svelte/TypeScript) has no automated test suite (no vitest/playwright configured) — frontend changes need manual verification in the running app.
