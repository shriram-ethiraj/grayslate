---
name: Tauri and Rust Backend Guidelines
description: Rules for using Tauri v2 APIs safely and effectively with Rust.
---

# Tauri and Rust Backend Guidelines

## Core Principles

- **Tauri v2 APIs:** Ensure we are using Tauri v2 IPC (`@tauri-apps/api/core` Invoke calls, not v1).
- **Rust Safety:** Follow strict memory safety protocols in Rust. Heavily utilize `Result<T, E>` for error handling instead of `unwrap()` or `expect()`.
- **Error Serialization:** Any Rust errors returned to the Svelte frontend via `#[tauri::command]` must implement `serde::Serialize`.
- **Async Commands:** Utilize async Rust functions for I/O operations (file system, network) to avoid blocking the main thread.
- **Security First:** When using Tauri APIs, assume the frontend is untrusted. Validate all payloads on the Rust side before execution, particularly if writing files or running binaries.
