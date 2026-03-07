import { invoke } from "@tauri-apps/api/core";
import { type as getOsType } from "@tauri-apps/plugin-os";

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

let reclaimRunning = false;
let reclaimSupported: boolean | undefined;

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

async function isReclaimSupportedPlatform(): Promise<boolean> {
  if (reclaimSupported !== undefined) {
    return reclaimSupported;
  }

  try {
    reclaimSupported = getOsType() !== "macos";
  } catch (error) {
    console.warn("[GC Pressure] Failed to detect platform", error);
    reclaimSupported = false;
  }

  return reclaimSupported;
}

/**
 * Reclaim stale JS heap memory after large file swaps on Windows/Linux.
 *
 * Allocates an ArrayBuffer sized proportionally to current system RAM usage,
 * writes to every 4 KiB page to force the OS to physically commit the buffer,
 * then releases it. This spikes V8's external-memory accounting just long
 * enough to provoke a major GC sweep after large editor teardowns.
 */
async function executeReclaim(): Promise<void> {
  if (reclaimRunning) {
    return;
  }

  reclaimRunning = true;

  try {
    const info = await invoke<MemoryInfo>("get_memory_info");

    if (info.available < MIN_AVAILABLE_RAM_FOR_PRESSURE) {
      return;
    }

    const usedMB = info.used / MB;
    const availableMB = info.available / MB;

    // 5% of system used RAM, floor 20 MB, cap 150 MB
    let pressureMB = Math.min(Math.max(usedMB * 0.05, 20), 150);
    pressureMB = Math.min(pressureMB, availableMB * 0.25);

    if (pressureMB <= 0) {
      return;
    }

    const pressureBytes = Math.floor(pressureMB * MB);

    // Yield to let Svelte + CodeMirror teardown complete
    await new Promise<void>((r) => setTimeout(r, 100));

    // Phase 1: V8 (Windows / Linux via WebView2) pressure.
    // Writing to every page forces the OS to physically commit all pages,
    // making the spike visible to V8's external-memory counter and
    // triggering a full Mark-Compact major GC.
    let buf: ArrayBuffer | null = new ArrayBuffer(pressureBytes);
    let view: Uint8Array | null = new Uint8Array(buf);
    for (let offset = 0; offset < view.length; offset += 4096) {
      view[offset] = 1;
    }
    view = null;
    buf = null;
  } catch (e) {
    console.warn("[GC Pressure]", e);
  } finally {
    reclaimRunning = false;
  }
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

  const shouldReclaim =
    previousDocBytes >= MIN_PEAK_DOC_BYTES &&
    shrinkBytes >= MIN_SHRINK_BYTES &&
    shrinkRatio >= MIN_SHRINK_RATIO &&
    postShrinkRatio <= MAX_POST_SHRINK_RATIO;

  if (!shouldReclaim) {
    return;
  }

  void isReclaimSupportedPlatform().then((supported) => {
    if (!supported) {
      return;
    }

    void executeReclaim();
  });
}
