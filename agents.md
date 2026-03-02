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
**> To know more about this topic, YOU MUST READ the `.agents/svelte-frontend/SKILL.md` file.**
*   **Embrace Svelte 5 Runes:** Exclusively use modern Svelte 5 signals (`$state`, `$derived`, `$effect`, `$props`). Avoid Svelte 4 legacy features.
*   **Strong Typing:** Do not use `any`. Use strict TypeScript interfaces.
*   **Vite Native:** Let Vite handle assets and bundling.

### 2. Editor Integration (CodeMirror 6)
**> To know more about core integration, YOU MUST READ the `.agents/codemirror-core/SKILL.md` file.**
**> To know more about custom extensions, YOU MUST READ the `.agents/editor-extensions/SKILL.md` file.**
*   Keep `EditorState` separate from Svelte's `$state` to avoid reactivity loops. 
*   Perform updates via `Transaction`s. 
*   **Performance:** Cap Lezer tree traversals to avoid freezing the main thread.

### 3. Desktop / Backend (Tauri v2 & Rust)
**> To know more about backend rules, YOU MUST READ the `.agents/tauri-backend/SKILL.md` file.**
*   Ensure usage of Tauri **v2** APIs.
*   Use `Result<T, E>` and `serde::Serialize` for returning Rust errors to Svelte.
*   Use async functions for I/O to avoid blocking. Validate all payloads.

### 4. Layout & CSS — Critical Rules
**> To know more about layout issues and fixes, YOU MUST READ the `.agents/layout-chain/SKILL.md` file.**

> **⚠️ Breaking these rules causes catastrophic virtualizer failures (CPU/memory spikes, app crash).**

*   **Never use `height: 100%` inside flex children.** Use `flex-1 min-h-0`.
*   **Every flex-column container and its flex children must have `min-h-0`.**
*   **`Sidebar.Inset` must always have `min-h-0 overflow-hidden`**.

### 5. Application Features & Core Libraries
**> To know more about the CSV Virtualizer, YOU MUST READ the `.agents/csv-architecture/SKILL.md` file.**
*   **Language Detection:** Uses a fast, heuristic synchronous pipeline.
*   **CSV Table View:** Uses a custom scroll virtualizer with a hard cap of 200 items. Limits exist to prevent browser max-height limits from rendering millions of DOM elements.
*   **Markdown Preview:** Parsed via `marked` and sanitized via `dompurify`, with custom bi-directional scroll synchronization.

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

