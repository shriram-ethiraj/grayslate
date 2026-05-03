---
name: memory-management
description: Memory reclamation strategy for file switching, GC pressure trick, and Rust sysinfo integration.
---

# Memory Management & GC Pressure Strategy

This document outlines the approach used in Grayslate to ensure memory is reclaimed promptly when switching between files.

## Primary Files

- `src/lib/editor/core/memory.ts`
- `src-tauri/src/commands/memory.rs`
- `src/lib/editor/components/EditorWrapper.svelte`

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
1. **Yield 100ms**: Wait for Svelte + CodeMirror teardown to finish. Old editor references must be unreachable before the GC sweep begins. 100ms is sufficient cross-platform — WebView2/V8 (Windows) needs the most headroom.
2. **Allocate**: `new ArrayBuffer(pressureBytes)` — spikes V8's heap.
3. **Phase 1 — Commit every page (V8 / Windows / Linux)**: Write `1` to every 4 KB offset in the buffer. This forces the OS to **physically commit** all pages into RSS, not just reserve virtual address space. V8 explicitly tracks ArrayBuffer backing-store bytes in its external-memory counter; the spike pushes past V8's `kExternalAllocationSoftLimit` (64 MB) and triggers a full Mark-Compact major GC.
4. **macOS Exception**: macOS uses WKWebView (JavaScriptCore), which handles memory differently and does not respond to the same `ArrayBuffer` pressure trick. Consequently, this manual GC trigger is **disabled on macOS**.
5. **Release**: Nullify the buffer. The GC sweeps it together with the old unreachable file state.

### When It Runs
`reclaimMemory()` (triggered via `requestFileOpenReclaim`) is called on **file open** in `EditorWrapper.svelte`. It runs only if specific "shrink" thresholds are met (e.g., swapping a large file for a significantly smaller one) to avoid unnecessary overhead during small operations.

## Current Implementation Notes

- The frontend first asks Rust for `available` and `used` memory via `get_memory_info`.
- `reclaimMemory()` skips the pressure trick if available RAM is below `MIN_AVAILABLE_RAM_FOR_PRESSURE`.
- The pressure buffer is sized from current system usage, then committed page-by-page in JavaScript.
- The function is intentionally called only after the old editor instance has had time to unmount.
- **Platform Check**: The logic is explicitly disabled on macOS via `isReclaimSupportedPlatform()`.

## 🦀 Rust Backend (`memory.rs`)
The `get_memory_info` command uses the `sysinfo` crate.

- **Fast Refresh**: Uses `System::new()` (not `new_all()`) and `refresh_memory()` to minimize CPU overhead.
- **Typed Return**: Returns a `Result<MemoryInfo, String>` with a `#[derive(Serialize)]` struct containing `available` and `used` memory.

## ⚠️ Critical Rules

1. **Always write to every 4 KB page** of the pressure buffer. Virtual-only `ArrayBuffer` allocations are unreliable.
2. **Never allocate more than 150 MB** in the pressure trick.
3. **Never allocate if available RAM < 500 MB** (avoid swap storms).
4. **Always yield 100ms** before allocating to let Svelte and CodeMirror teardown complete.
5. **Single pass only.** Do NOT use multiple allocation passes.
6. **Threshold-based execution.** Only run the reclaim when replacing a large document with a significantly smaller one, as defined by `MIN_PEAK_DOC_BYTES`, `MIN_SHRINK_BYTES`, and `MIN_SHRINK_RATIO`.
7. **macOS is unsupported.** Do not attempt to trigger manual GC on macOS as the standard V8 pressure techniques do not apply to JavaScriptCore in the same way.
