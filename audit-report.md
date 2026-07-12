# Grayslate codebase audit report

Repository: `<local repository path>`

## Scope

This review covered:

- Svelte 5 rune usage, prop typing, and event syntax
- TypeScript safety and Tauri IPC typing
- Rust / Tauri command design, error handling, async usage, and dependencies
- Performance, main-thread work, worker lifecycle, and cleanup
- Dead code, command reachability, code organization, and security

## Validation note

Automated baseline validation could not be completed in this environment because `pnpm run check` depends on PowerShell 6+ (`pwsh.exe`), which is not available here. Manual code review findings are below.

## What looked good

- The runtime Svelte components are already rune-first: no production `export let`, no legacy `$:` reactive statements, and no legacy `on:` event syntax were found in app code.
- Frontend code is generally strongly typed; no production `any` usage was found in the application sources reviewed.
- Tauri commands are registered consistently and every registered command appears to have a frontend caller.
- Capability files are relatively narrow; the main security concern is the disabled CSP rather than obviously over-broad permissions.

---

## Critical

### 1. Arbitrary local-file reads are possible through the Tauri command boundary

- **Files:** `src-tauri\src\commands\file.rs:39-51`, `src\lib\editor\components\EditorWrapper.svelte:430-432`
- **Problem:** `read_file_content` accepts an arbitrary string path and reads it directly with `std::fs::metadata` / `std::fs::read_to_string` without validating that the path is absolute, canonical, user-approved, or inside an allowed root.
- **Why it matters:** Tauri should treat the frontend as untrusted. If any webview script execution bug occurs, the attacker can invoke this command directly and exfiltrate arbitrary readable local files.
- **Concrete fix:** Add a read-path validator similar to the write-path validator, canonicalize the path, and enforce an allowlist such as user-selected files or configured notes roots. A stronger design is to keep file selection in Rust and return opaque handles/tokens instead of accepting raw paths from the frontend.

### 2. Webview CSP is disabled

- **File:** `src-tauri\tauri.conf.json:25-27`
- **Problem:** `"csp": null` disables Content Security Policy protection entirely.
- **Why it matters:** In a Tauri app, XSS is especially dangerous because injected scripts can reach privileged Rust commands. This setting sharply increases the blast radius of any markup injection or third-party dependency issue.
- **Concrete fix:** Define a strict CSP for the packaged app, for example starting with `default-src 'self'; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'`, then tighten further based on actual asset/runtime needs.

---

## Warning

### 3. Large synchronous file reads run inside an async command and return huge IPC payloads

- **Files:** `src-tauri\src\commands\file.rs:40-51`, `src\lib\editor\components\EditorWrapper.svelte:430-432`
- **Problem:** `read_file_content` is declared `async` but performs synchronous filesystem calls, then sends the full file contents back over IPC. The current limit is 200 MB.
- **Why it matters:** Reading and serializing very large files this way can block a Tauri runtime thread, spike memory, and create slow UI transitions when the entire payload crosses the Rust↔webview boundary at once.
- **Concrete fix:** Move the read into `spawn_blocking` (as already done for writes), and consider chunked reads / progressive loading for large files. If full-text mode must stay, lower the threshold for eager IPC transfer and gate truly large files behind a different loading path.

### 4. Production code still contains panic paths and silently swallowed native-window errors

- **Files:** `src-tauri\src\lib.rs:42-43`, `src-tauri\src\window\mod.rs:20-48`
- **Problem:** The app startup path ends with `.expect("error while running tauri application")`, and macOS window styling uses `.expect("NSWindow.contentView() should not be null")`. The `with_webview(...).ok()` call then discards any returned error.
- **Why it matters:** These are production paths. A startup or native-window edge case can crash the app instead of failing gracefully, and swallowed errors make field diagnostics harder.
- **Concrete fix:** Replace `.expect(...)` with explicit error handling that logs context and returns a `tauri::Result` where possible. In `apply_macos_window_styling`, guard `contentView()` without panicking and log or propagate `with_webview` failures instead of dropping them with `.ok()`.

### 5. CSV worker lifecycle does not handle worker crashes or message deserialization failures

- **Files:** `src\lib\editor\components\csv\CsvTableView.svelte:195-248`, `src\lib\editor\components\csv\CsvTableView.svelte:362-418`
- **Problem:** `createTableWorker()` only wires `onmessage`. There is no `onerror` / `onmessageerror` handling, and initialization failures are only written to `console.error`.
- **Why it matters:** If the worker crashes, module loading fails, or a malformed message crosses the boundary, pending promises can remain unresolved and the table UI can stall indefinitely.
- **Concrete fix:** Add `tableWorker.onerror` and `tableWorker.onmessageerror` handlers that call `resetPendingRequests(...)`, tear down the worker, and surface the failure to the user. Also consider attaching a worker-generation token so only messages from the current worker instance are honored.

### 6. Markdown preview does full parse + sanitize work on every content change on the main thread

- **File:** `src\lib\editor\components\markdown\MarkdownPreview.svelte:100-227`
- **Problem:** `renderMarkdown(content)` constructs a new `Marked` instance, walks tokens, renders HTML, and sanitizes with DOMPurify inside `let htmlPreview = $derived(renderMarkdown(content));`.
- **Why it matters:** This is correct from a dependency standpoint, but expensive for large markdown documents because it re-runs synchronously for every text change and competes with CodeMirror/editor work on the main thread.
- **Concrete fix:** Debounce preview recomputation, memoize by content/version, or offload parsing to a worker while keeping sanitization and DOM insertion on the main thread.

### 7. `Editor.svelte` weakly types its props and relies on repeated casts

- **File:** `src\lib\editor\components\Editor.svelte:21-29`, `src\lib\editor\components\Editor.svelte:35`, `src\lib\editor\components\Editor.svelte:54`, `src\lib\editor\components\Editor.svelte:71`, `src\lib\editor\components\Editor.svelte:93`, `src\lib\editor\components\Editor.svelte:102`, `src\lib\editor\components\Editor.svelte:110`, `src\lib\editor\components\Editor.svelte:137`, `src\lib\editor\components\Editor.svelte:172`
- **Problem:** The component does not declare a dedicated props interface, and `session` is repeatedly cast with `session as ManagedEditorSession`.
- **Why it matters:** This undermines the otherwise strong TypeScript story and makes it easier for the component contract to drift without compiler help.
- **Concrete fix:** Define `interface EditorProps { ... session?: ManagedEditorSession; }` and switch to `$props<EditorProps>()`, so the default session and all downstream helpers stay fully typed without casts.

### 8. Some Tauri invokes are not fully typed and one fire-and-forget IPC call ignores rejection

- **Files:** `src\lib\components\Titlebar.svelte:309-311`, `src\lib\editor\components\EditorWrapper.svelte:495-507`
- **Problem:** `invoke("set_menu_word_wrap", { checked: editorState.wordWrap });` is untyped and its returned promise is ignored. `write_file_content` is also invoked without an explicit `invoke<void>(...)` return type.
- **Why it matters:** These are easy places for command-signature drift to slip in silently, especially in a codebase that is otherwise careful about typed IPC (`invoke<string>`, `invoke<UpdateCheckResponse>`, `invoke<MemoryInfo>`).
- **Concrete fix:** Introduce small typed wrappers in a shared IPC module (for example `setMenuWordWrap(checked: boolean): Promise<void>`), or at minimum use `void invoke<void>(...)` plus `.catch(...)` where failures should not be surfaced synchronously.

### 9. Markdown preview cleanup contains dead assignments, and parser failures are flattened to a generic fallback

- **File:** `src\lib\editor\components\markdown\MarkdownPreview.svelte:222-287`
- **Problem:** The `catch` returns `"<p>Error parsing markdown</p>"` for all failures, and `onDestroy` assigns `content = ""` and `htmlPreview = ""` even though one is a prop and the other is a derived value being torn down anyway.
- **Why it matters:** The dead assignments make teardown intent misleading, and the broad fallback hides useful debugging information for markdown parsing/rendering failures.
- **Concrete fix:** Remove the `content = ""` and `htmlPreview = ""` assignments. Keep cleanup focused on unregistering DOM state. For parse failures, at least log the error in development or capture it in component state so the failure mode remains diagnosable.

### 10. Editor lifecycle transitions depend on fixed `setTimeout` delays instead of Svelte lifecycle completion

- **File:** `src\lib\editor\components\EditorWrapper.svelte:389`, `src\lib\editor\components\EditorWrapper.svelte:438`, `src\lib\editor\components\EditorWrapper.svelte:442`, `src\lib\editor\components\EditorWrapper.svelte:463`
- **Problem:** File creation/open flows rely on hard-coded 0 ms / 10 ms timeouts to wait for repaint, DOM updates, and editor disposal.
- **Why it matters:** These timing assumptions are race-prone under load, on slower machines, or when the browser/event loop behaves differently than expected.
- **Concrete fix:** Prefer `tick()` / an explicit lifecycle barrier for DOM updates, and keep disposal state transitions explicit rather than clock-based.

### 11. `serde_json` appears unused in the Rust backend

- **File:** `src-tauri\Cargo.toml:25-29`
- **Problem:** `serde_json = "1"` is declared, but no Rust source under `src-tauri\src` references it.
- **Why it matters:** Unused dependencies increase compile time and maintenance surface.
- **Concrete fix:** Remove `serde_json` from `Cargo.toml` if it is not required by planned near-term work.

---

## Suggestion

### 12. Two core frontend components are carrying too many responsibilities

- **Files:** `src\lib\editor\components\EditorWrapper.svelte:1-808`, `src\lib\editor\components\csv\CsvTableView.svelte:1-760`
- **Problem:** `EditorWrapper.svelte` owns document identity, file I/O, language detection, loader orchestration, CSV-mode coordination, and menu event handling. `CsvTableView.svelte` owns worker lifecycle, virtualization, mutation plumbing, sizing state, and interaction handling.
- **Why it matters:** Both components are workable, but they are becoming hard to reason about and hard to test in isolation.
- **Concrete fix:** Extract shared file/session orchestration into a typed service/composable module, and split CSV worker transport/state from the rendering component.

### 13. IPC contracts are scattered instead of centralized

- **Files:** `src\lib\state\appMenu.svelte.ts:15-37`, `src\lib\editor\core\memory.ts:4-7`, `src\lib\files\notesRoot.ts:32-38`, `src\lib\editor\components\EditorWrapper.svelte:430-432`, `src\lib\editor\components\EditorWrapper.svelte:507`
- **Problem:** Rust command names and payload/response shapes are repeated in several unrelated frontend modules.
- **Why it matters:** Scattered IPC contracts make refactors riskier and encourage inconsistent typing quality from one caller to the next.
- **Concrete fix:** Centralize frontend Tauri command wrappers in something like `src\lib\tauri\ipc.ts` and export typed functions for each command.

### 14. The markdown/context-menu cleanup pattern is good overall; standardize it as the project norm

- **Files:** `src\lib\editor\components\EditorContextMenu.svelte:82-132`, `src\lib\editor\components\markdown\MarkdownPreviewContextMenu.svelte:67-104`, `src\lib\editor\components\csv\CsvContextMenu.svelte:84-105`
- **Problem:** There is no bug here; this is a consistency opportunity.
- **Why it matters:** These components already use `$effect` cleanup well for transient listeners and hotkeys. Reusing this pattern consistently will help prevent the kind of lifecycle drift seen elsewhere.
- **Concrete fix:** Treat these listener-registration patterns as the reference implementation for future ephemeral UI surfaces.

---

## Priority order

1. Fix the arbitrary read-path trust boundary in `read_file_content`.
2. Replace `csp: null` with a production CSP.
3. Harden backend/runtime failure handling (`expect`, swallowed errors, worker crash handling).
4. Reduce large main-thread / large-IPC work for file open and markdown preview.
5. Tighten typed IPC and split oversized orchestration components before they grow further.

## Recommended next steps

- Implement the two critical fixes first: read-path validation and a real CSP.
- Then harden worker error handling and remove production panic paths.
- After `pwsh.exe` is available, run:
  - `pnpm run check`
  - `pnpm tauri build`
