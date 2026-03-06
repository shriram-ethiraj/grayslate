import { invoke } from "@tauri-apps/api/core";

interface MemoryInfo {
  total: number;
  available: number;
  used: number;
  /** RSS of this process in bytes. */
  process_used: number;
}

const MB = 1024 * 1024;

// Skip GC pressure if the app process is already using less than this.
const PROCESS_RSS_THRESHOLD = 200 * MB;

/**
 * Reclaim stale JS heap memory after switching away from a large file.
 *
 * Allocates an ArrayBuffer sized proportionally to the system's current RAM
 * usage, writes to every 4 KB page to force the OS to physically commit
 * every page into RSS, then releases the buffer. This serves two purposes:
 *
 * 1. **V8 heap pressure** — the sudden spike in live heap pushes past V8's
 *    dynamic allocation limit, triggering a full Mark-Compact major GC that
 *    sweeps the now-unreachable CodeMirror state, old file string, Lezer
 *    syntax tree, and view decorations.
 *
 * 2. **OS-level signal** — physically committed pages are visible to the OS
 *    memory manager. On Windows, WebView2 receives memory pressure
 *    callbacks; on macOS/Linux the kernel reclaims the pages aggressively
 *    after release. This is why GC-E consistently reduces RSS across all
 *    three platforms while virtual-only allocations are unreliable.
 *
 * Pressure is computed from **system used RAM** — not from file size or
 * content length — because the actual heap bloat includes CodeMirror's
 * 3-5× overhead, DOM nodes, and internal V8 bookkeeping that can't be
 * estimated from `string.length` alone.
 *
 * Called automatically after every file open. The 100 ms yield gives Svelte
 * and CodeMirror time to tear down the previous editor instance so the old
 * objects are unreachable before the GC sweep begins.
 */
export async function reclaimMemory(): Promise<void> {
  try {
    const info = await invoke<MemoryInfo>("get_memory_info");

    // Skip if the process is already lean — nothing significant to reclaim
    if (info.process_used < PROCESS_RSS_THRESHOLD) return;

    const usedMB = info.used / MB;
    const availableMB = info.available / MB;

    // 5% of system used RAM, floor 20 MB, cap 150 MB
    let pressureMB = Math.min(Math.max(usedMB * 0.05, 20), 150);

    // If system RAM is tight (< 500 MB free), limit to 25% of available
    if (availableMB < 500) {
      pressureMB = Math.min(pressureMB, availableMB * 0.25);
    }

    if (pressureMB <= 0) return;

    const pressureBytes = Math.floor(pressureMB * MB);

    // Yield to let Svelte + CodeMirror teardown complete
    await new Promise<void>((r) => setTimeout(r, 100));

    // Allocate and commit every 4 KB page
    let buf: ArrayBuffer | null = new ArrayBuffer(pressureBytes);
    const view = new Uint8Array(buf);
    for (let offset = 0; offset < view.length; offset += 4096) {
      view[offset] = 1;
    }
    buf = null;
  } catch (e) {
    console.warn("[GC Pressure]", e);
  }
}
