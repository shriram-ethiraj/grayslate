# Memory Management & GC Pressure Strategy

This document outlines the approach used in Grayslate to ensure memory is reclaimed promptly when switching between files.

## 🧠 The Problem
JavaScript engines (V8, JavaScriptCore) are often lazy. Large strings and the corresponding CodeMirror state (document trees, syntax trees) can occupy 5–10× the raw file size in the heap. When a user opens a new file, the old state becomes unreachable, but the engine might wait until the heap is nearly full before running a **Major GC** pass.

In a desktop app context, this results in:
1. High "Used RAM" in Task Manager/Activity Monitor.
2. Potential UI stutters if the GC eventually triggers during an interaction.

## 🛠️ The Solution: Full-Page-Commit GC Pressure
We use a single function `reclaimMemory()` in `src/lib/editor/core/memory.ts`, called automatically after every file open.

### How Pressure Is Sized
Pressure is computed from **system used RAM** (via Rust `sysinfo` crate), NOT from file content length. Content length is an unreliable proxy because the actual heap bloat includes CodeMirror's 3–5× overhead, DOM nodes, and V8 bookkeeping.

- **Formula**: `5% of system used RAM`, floor **20 MB**, cap **150 MB**.
- **Safety**: If available RAM < 500 MB, reduce to at most `25% of available`.
- If pressure computes to ≤ 0 after safety, we abort.

### How It Works
1. **Yield 100ms**: Wait for Svelte + CodeMirror teardown to finish. Old editor references must be unreachable before the GC sweep begins. 100ms is sufficient cross-platform — WebView2/V8 (Windows) needs the most headroom; WKWebView/JSC (macOS) and WebKitGTK/JSC (Linux) are faster.
2. **Allocate**: `new ArrayBuffer(pressureBytes)` — spikes V8's heap.
3. **Commit every page**: Write `1` to every 4 KB offset in the buffer. This forces the OS to **physically commit** all pages into RSS (resident set size), not just reserve virtual address space. This is the critical difference from a naive allocation — it makes the pressure visible to both:
   - **V8's heap accounting** → triggers a full Mark-Compact major GC.
   - **The OS memory manager** → on Windows, WebView2 receives memory pressure callbacks; on macOS/Linux, the kernel reclaims the pages aggressively after release.
4. **Release**: Nullify the buffer. The GC now sees the full spike as reclaimable and sweeps it along with the old unreachable file state.

### When It Runs
`reclaimMemory()` is called on **every file open** in `EditorWrapper.svelte`, unconditionally. There is no content-length threshold — the function is cheap when the system isn't bloated (the 20 MB floor is harmless) and essential when it is.

## 🦀 Rust Backend (`memory.rs`)
The `get_memory_info` command uses the `sysinfo` crate.

- **Fast Refresh**: Uses `System::new()` (not `new_all()`) and `refresh_memory()` to minimize CPU overhead. `new_all()` enumerates all processes, CPUs, disks, and NICs — far too expensive for a simple RAM check.
- **Typed Return**: Returns a `Result<MemoryInfo, String>` with a `#[derive(Serialize)]` struct containing `total`, `available`, and `used` (bytes).

## ⚠️ Critical Rules

1. **Always write to every 4 KB page** of the pressure buffer. Virtual-only `ArrayBuffer` allocations are unreliable — the OS may lazily back them and V8's GC heuristics may not "see" uncommitted memory.
2. **Never allocate more than 150 MB** in the pressure trick.
3. **Never allocate if available RAM < 500 MB** (avoid swap storms).
4. **Always yield 100ms** before allocating to let Svelte and CodeMirror teardown complete.
5. **Single pass only.** Do NOT use multiple allocation passes — rapid successive allocations confuse V8's heap accounting in WebView2 and cause it to *retain* memory instead of releasing it. This was tested and confirmed during development.
6. **Do NOT re-introduce content-length-based sizing.** It was tested and proved unreliable — the actual heap bloat depends on CodeMirror overhead that can't be estimated from `string.length`.
