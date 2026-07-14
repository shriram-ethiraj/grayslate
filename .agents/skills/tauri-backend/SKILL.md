---
name: tauri-backend
description: Rules for using Tauri v2 APIs safely and effectively with Rust in Grayslate.
---

# Tauri and Rust Backend Guidelines

## Current Backend Surface In Grayslate

Primary files:

- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/file.rs`
- `src-tauri/src/commands/naming.rs`
- `src-tauri/src/commands/search.rs`
- `src-tauri/src/commands/memory.rs`
- `src-tauri/src/detection.rs` (re-export shim over `crates/grayslate-langdetect/`)
- `src-tauri/src/menu/mod.rs`
- `src-tauri/src/window/mod.rs`

Key commands exposed to the frontend include:

- `read_file_content`
- `cancel_file_read`
- `write_file_content`
- `delete_file`
- `rename_file`
- `duplicate_file`
- `duplicate_local_file_as_slate`
- `detect_language`
- `save_untitled_slate`
- `suggest_slate_name`
- `suggest_name_for_file`
- `get_recent_files`
- `search_sidebar_files`
- `get_memory_info`
- `set_menu_word_wrap`

Current implementation notes:

- File reads are validated in Rust before returning content to the frontend.
- `read_file_content` is cancellable per window and returns raw UTF-8 bytes via `tauri::ipc::Response`.
- The enforced file-open limit is currently 200 MB.
- `detect_language` runs the family-first detection pipeline: extension → shebang → strong structural → family classification → family-gated scoring → disambiguation, abstaining when no confident match is found.
- macOS native menu wiring is handled in Rust; Windows and Linux use the in-window menu implementation.
- The app builder uses Tauri v2 plugins for window state, OS info, opener, dialog, and clipboard.

## Backend-Driven Recent Files Updates

`src-tauri/src/commands/mod.rs` defines:

```rust
pub const RECENT_FILES_UPDATED_EVENT: &str = "files://recent-updated";
```

This event is emitted by the backend after every file operation that should refresh the sidebar's recent-files data.

Current emit sites include tracked file activity:

- `read_file_content` — after inserting a previously untracked open
- `write_file_content`
- `delete_file`
- `rename_file`
- `duplicate_local_file_as_slate`
- `duplicate_file`
- `save_untitled_slate`

The frontend sidebar listens for this event and refreshes itself. Do not add mirror frontend emits for the same file operations unless there is a very specific reason and the event contract is being changed deliberately.

## File Read Tracking

`read_file_content` owns first-open tracking:

1. validate and read the file
2. classify the file source
3. insert its `tracked_files` row with `ON CONFLICT DO NOTHING`
4. emit `RECENT_FILES_UPDATED_EVENT` only if the row was inserted
5. return raw bytes to the frontend

This ensures a newly opened local file is stored with source `local` and is
available to the Local sidebar tab. Reopening any already tracked slate or
local file leaves all database fields and recency timestamps untouched.

Content saves emit `RECENT_FILES_UPDATED_EVENT` with the `"saved"` payload so
the sidebar can immediately refresh even when an opened-file reorder freeze is
active.

## Naming Command Contracts

`src-tauri/src/commands/naming.rs` is the command boundary for save/rename suggestion flows.

Important behavior:

- `save_untitled_slate` returns both `path` and `detectedLanguage`
- `suggest_slate_name` returns both `filename` and `detectedLanguage`
- `suggest_name_for_file(path)` reads a bounded disk sample and intentionally does **not** trust the file extension as a naming hint

Keep command handlers thin:

- naming logic belongs in `crates/grayslate-langnaming/` (see `src-tauri/src/naming.rs` shim)
- detection logic belongs in `crates/grayslate-langdetect/` (see `src-tauri/src/detection.rs` shim)
- command modules should orchestrate I/O, storage updates, and event emission

## Core Principles

- **Tauri v2 APIs:** Ensure we are using Tauri v2 IPC (`@tauri-apps/api/core` invoke calls, not v1).
- **Rust Safety:** Prefer `Result<T, E>` and explicit error propagation over `unwrap()` / `expect()`.
- **Error Serialization:** Rust errors returned through `#[tauri::command]` must be serializable or stringified consistently.
- **Async Commands:** Use async Rust functions for I/O-heavy work and `spawn_blocking` where synchronous filesystem work is unavoidable.
- **Security First:** Assume the frontend is untrusted. Validate all payloads, especially file paths, file names, and write targets.

## Safe Change Checklist

- Keep command modules thin and push business logic into plain Rust modules.
- Preserve backend ownership of `RECENT_FILES_UPDATED_EVENT`.
- Preserve insert-only first-open tracking inside `read_file_content`.
- Re-run `cargo check` or `cargo test --manifest-path src-tauri/Cargo.toml` after backend changes.
- Re-run `pnpm run check` if the IPC contract changed and Svelte types or callers were updated.
