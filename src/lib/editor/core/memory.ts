import { invoke } from "@tauri-apps/api/core";
import { reportGcDebugFinding } from "$lib/state/gc-debug-controls.svelte";
import type {
  GcDebugDecisionStage,
  GcReclaimTriggerSource,
} from "$lib/state/gc-debug.svelte";

interface MemoryInfo {
  available: number;
  used: number;
}

const MB = 1024 * 1024;
const UTF16_BYTES_PER_CODE_UNIT = 2;

// Avoid pressure allocation when available RAM is already tight.
const MIN_AVAILABLE_RAM_FOR_PRESSURE = 500 * MB;
const MIN_PEAK_DOC_BYTES = 16 * MB;
const MIN_SHRINK_BYTES = 8 * MB;
const MIN_SHRINK_RATIO = 0.35;
const MAX_POST_SHRINK_RATIO = 0.7;

type ShrinkMetrics = {
  shrinkBytes: number;
  shrinkRatio: number;
  postShrinkRatio: number;
};

let currentDocBytes = 0;
let reclaimRunning = false;
let lastReclaimAt = 0;

function docLengthToBytes(length: number): number {
  return Math.max(0, length) * UTF16_BYTES_PER_CODE_UNIT;
}

function getShrinkMetrics(
  previousDocBytes: number,
  nextDocBytes: number,
): ShrinkMetrics {
  const shrinkBytes = previousDocBytes - nextDocBytes;
  const shrinkRatio = previousDocBytes > 0 ? shrinkBytes / previousDocBytes : 0;
  const postShrinkRatio =
    previousDocBytes > 0 ? nextDocBytes / previousDocBytes : 0;

  return {
    shrinkBytes,
    shrinkRatio,
    postShrinkRatio,
  };
}

/**
 * Reclaim stale JS heap memory after large file swaps.
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
 * The 100 ms yield gives Svelte and CodeMirror time to tear down unreachable
 * objects before the GC sweep begins.
 */
async function executeReclaim(source: GcReclaimTriggerSource): Promise<void> {
  if (reclaimRunning) {
    reportGcDebugFinding({
      timestamp: Date.now(),
      stage: "skipped",
      source,
      reason: "GC trigger skipped because another reclaim run is already in progress.",
      currentDocBytes,
      lastReclaimAt,
    });
    return;
  }

  reclaimRunning = true;

  reportGcDebugFinding({
    timestamp: Date.now(),
    stage: "running",
    source,
    reason: `Running GC trigger from ${source}.`,
    currentDocBytes,
    lastReclaimAt,
    lastError: "",
  });

  try {
    const info = await invoke<MemoryInfo>("get_memory_info");

    if (info.available < MIN_AVAILABLE_RAM_FOR_PRESSURE) {
      reportGcDebugFinding({
        timestamp: Date.now(),
        stage: "skipped",
        source,
        reason: "GC trigger skipped because available system RAM is below the safety threshold.",
        currentDocBytes,
        lastReclaimAt,
        lastSystemUsed: info.used,
        lastAvailable: info.available,
        lastPressureBytes: 0,
      });
      return;
    }

    const usedMB = info.used / MB;
    const availableMB = info.available / MB;

    // 5% of system used RAM, floor 20 MB, cap 150 MB
    let pressureMB = Math.min(Math.max(usedMB * 0.05, 20), 150);
    pressureMB = Math.min(pressureMB, availableMB * 0.25);

    if (pressureMB <= 0) {
      reportGcDebugFinding({
        timestamp: Date.now(),
        stage: "skipped",
        source,
        reason: "GC trigger skipped because the pressure allocation resolved to zero after safety limits.",
        currentDocBytes,
        lastReclaimAt,
        lastSystemUsed: info.used,
        lastAvailable: info.available,
        lastPressureBytes: 0,
      });
      return;
    }

    const pressureBytes = Math.floor(pressureMB * MB);

    // Yield to let Svelte + CodeMirror teardown complete
    await new Promise<void>((r) => setTimeout(r, 100));

    // Allocate and commit every 4 KB page
    let buf: ArrayBuffer | null = new ArrayBuffer(pressureBytes);
    let view: Uint8Array | null = new Uint8Array(buf);
    for (let offset = 0; offset < view.length; offset += 4096) {
      view[offset] = 1;
    }
    view = null;
    buf = null;
    lastReclaimAt = Date.now();

    reportGcDebugFinding({
      timestamp: Date.now(),
      stage: "completed",
      source,
      reason: `GC trigger completed from ${source}.`,
      currentDocBytes,
      lastReclaimAt,
      lastSystemUsed: info.used,
      lastAvailable: info.available,
      lastPressureBytes: pressureBytes,
    });
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e);
    reportGcDebugFinding({
      timestamp: Date.now(),
      stage: "failed",
      source,
      reason: "GC trigger failed while applying memory pressure.",
      currentDocBytes,
      lastReclaimAt,
      lastError: errorMessage,
    });
    console.warn("[GC Pressure]", e);
  } finally {
    reclaimRunning = false;
  }
}

export async function reclaimMemory(): Promise<void> {
  await executeReclaim("manual");
}

export function requestFileOpenReclaim(
  previousDocLength: number,
  nextDocLength: number,
): void {
  const previousDocBytes = docLengthToBytes(previousDocLength);
  const nextDocBytes = docLengthToBytes(nextDocLength);
  const { shrinkBytes, shrinkRatio, postShrinkRatio } = getShrinkMetrics(
    previousDocBytes,
    nextDocBytes,
  );
  currentDocBytes = nextDocBytes;

  let stage: GcDebugDecisionStage;
  let reason: string;

  if (previousDocBytes < MIN_PEAK_DOC_BYTES) {
    stage = "skipped";
    reason = "File-open fallback skipped because the previous document is below the minimum reclaim threshold.";
  } else if (shrinkBytes < MIN_SHRINK_BYTES) {
    stage = "skipped";
    reason = "File-open fallback skipped because the document swap did not release enough content.";
  } else if (shrinkRatio < MIN_SHRINK_RATIO) {
    stage = "skipped";
    reason = "File-open fallback skipped because the document swap shrink ratio is below the configured minimum.";
  } else if (postShrinkRatio > MAX_POST_SHRINK_RATIO) {
    stage = "skipped";
    reason = "File-open fallback skipped because too much of the previous document is still retained after the swap.";
  } else {
    stage = "running";
    reason = "File-open fallback passed shrink thresholds and will attempt reclaim.";
  }

  reportGcDebugFinding({
    timestamp: Date.now(),
    stage,
    source: "file-open",
    reason,
    currentDocBytes,
    lastReclaimAt,
    lastShrinkBytes: shrinkBytes,
    lastShrinkRatio: shrinkRatio,
    lastPostShrinkRatio: postShrinkRatio,
  });

  if (stage === "running") {
    void executeReclaim("file-open");
  }
}
