# 🤖 AI Agent Implementation Guidelines: Grayslate

Welcome to the Grayslate project. This document serves as a "production-grade" reference for any AI agents, coding assistants (like Cursor, Copilot, or Claude), and developers working on this repository.

## 🎯 Project Overview
**Grayslate** is a lightweight, cross-platform developer scratchpad. It features built-in functions for handling data formats like JSON and CSV, and allows users to add their own custom functions. Notes are auto-saved, can be custom-named by the user, and are automatically synced to Git.

## 🛠️ Tech Stack & Constraints
- **Desktop Framework:** Tauri v2
- **Backend Languages:** Rust
- **Frontend Framework:** Svelte 5 (SvelteKit with Static Adapter)
- **Frontend Language:** TypeScript
- **Editor Engine:** CodeMirror 6
- **Bundler:** Vite
- **Package Manager:** pnpm

---

## 📐 Architecture & Coding Standards

### 1. Frontend (Svelte 5 & TypeScript)
*   **Embrace Svelte 5 Runes:** Exclusively use modern Svelte 5 signals (Runes).
    *   Use `$state()` for reactive state.
    *   Use `$derived()` for computed values.
    *   Use `$effect()` for side effects.
    *   Use `$props()` instead of `export let` for component inputs.
    *   Avoid legacy Svelte 4 features (`$:`, legacy slot architecture). Opt for Svelte 5 `{#snippet}` when handling template injection.
*   **Strong Typing:** Do not use `any`. Define strictly typed interfaces and types for all component props, Tauri IPC payloads, and CodeMirror extensions.
*   **Vite Native:** Keep assets optimized. Import static assets cleanly and let Vite handle caching and bundling.

### 2. Editor Integration (CodeMirror 6)
*   CodeMirror 6 is highly modular. Do not import massive monolithic packages.
*   Use `@codemirror/state` and `@codemirror/view` correctly.
*   Keep the CodeMirror `EditorState` conceptually separated from Svelte's `$state` unless explicitly synchronizing document content. Avoid deep reactivity loops between the two.
*   Dispatch `Transaction` objects cleanly for editor updates rather than violently replacing the document text.

### 3. Desktop / Backend (Tauri v2 & Rust)
*   **Tauri v2 APIs:** Ensure we are using Tauri v2 IPC (`@tauri-apps/api/core` Invoke calls, not v1).
*   **Rust Safety:** Follow strict memory safety protocols in Rust. Heavily utilize `Result<T, E>` for error handling instead of `unwrap()` or `expect()`.
*   **Error Serialization:** Any Rust errors returned to the Svelte frontend via `#[tauri::command]` must implement `serde::Serialize`.
*   **Async Commands:** Utilize async Rust functions for I/O operations (file system, network) to avoid blocking the main thread.

### 4. Application Features & Core Libraries
*   **Supported Languages:** The editor explicitly supports and provides syntax/tooling for `Text`, `JSON`, `JavaScript/TypeScript`, `Python`, `CSV`, and `Markdown`.
*   **Language Detection:** Automatic file type detection is primarily handled by `@vscode/vscode-languagedetection` (a TensorFlow ML model) running locally, preceded by fast custom heuristics for common data formats (JSON, CSV, Markdown) to ensure performance.
*   **CSV Table View:** For structured data, the app uses a virtualized spreadsheet mode powered by `@tanstack/svelte-table` for headless data grid logic and `@tanstack/svelte-virtual` for performant DOM virtualization of large datasets.
*   **Markdown Preview:** Markdown is parsed into an AST and converted to HTML using `marked`, then heavily sanitized using `dompurify`. Custom renderer hooks inject `data-line` attributes to achieve seamless, bi-directional scroll synchronization between the editor and the preview pane.

---

## 📝 Agent Instructions

When generating code or proposing architectural changes, adhere to the following rules:

1.  **Reflect the Stack:** Always provide solutions in standard Rust (Edition 2021) and Svelte 5.
2.  **No Hallucinations:** Check Tauri 2.0 and CodeMirror 6 documentation bounds. These APIs have changed significantly from their previous major versions; double-check the syntax before writing out plugin configs or IPC logic.
3.  **Readability over Cleverness:** Write code that is maintainable. Comment complex regex, complex Rust lifetimes, and custom CodeMirror state fields heavily.
4.  **Security First:** When using Tauri APIs, assume the frontend is untrusted. Validate all payloads on the Rust side before execution, particularly if writing files or running binaries.
5.  **Conciseness:** Provide the exact code block needed to fix the issue. Avoid unnecessary pleasantries or overly long explanations unless asked to explain the architectural choice.

## 🚀 Commits & Workflow
*   Keep `.gitignore` respected (e.g., `node_modules`, `target`, `.svelte-kit`).
*   Verify code works with `pnpm run check` (runs `svelte-check`) and compiles with `pnpm run tauri build`.
