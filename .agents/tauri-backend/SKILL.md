---
name: Tauri and Rust Backend Guidelines
description: Rules for using Tauri v2 APIs safely and effectively with Rust.
---

# Tauri and Rust Backend Guidelines

## Current Backend Surface In Grayslate

Primary files:

- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/file.rs`
- `src-tauri/src/commands/memory.rs`
- `src-tauri/src/menu/mod.rs`
- `src-tauri/src/window/mod.rs`

Commands currently exposed to the frontend:

- `read_file_content`
- `get_memory_info`
- `set_menu_word_wrap`

Current implementation notes:

- File reads are validated in Rust before returning content to the frontend.
- The current enforced file size limit is 200 MB.
- macOS native menu wiring is handled in Rust; Windows and Linux use the in-window menu implementation.
- The app builder uses Tauri v2 plugins for window state, OS info, opener, dialog, and clipboard.

## Core Principles

- **Tauri v2 APIs:** Ensure we are using Tauri v2 IPC (`@tauri-apps/api/core` Invoke calls, not v1).
- **Rust Safety:** Follow strict memory safety protocols in Rust. Heavily utilize `Result<T, E>` for error handling instead of `unwrap()` or `expect()`.
- **Error Serialization:** Any Rust errors returned to the Svelte frontend via `#[tauri::command]` must implement `serde::Serialize`.
- **Async Commands:** Utilize async Rust functions for I/O operations (file system, network) to avoid blocking the main thread.
- **Security First:** When using Tauri APIs, assume the frontend is untrusted. Validate all payloads on the Rust side before execution, particularly if writing files or running binaries.
