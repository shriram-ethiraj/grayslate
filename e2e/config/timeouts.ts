/**
 * The only place timeout literals live.
 *
 * Before this module the suite carried ad-hoc `5_000` / `10_000` / `20_000` /
 * `30_000` values scattered across specs, so nobody could tell whether a number
 * encoded "a click should repaint" or "a debounced Rust write should land".
 * Name the intent, not the duration.
 */
export const TIMEOUTS = {
  /** One UI reaction: a click's DOM effect, a dialog opening or closing. */
  ui: 10_000,

  /** A transient tooltip/menu overlay dismissing after the pointer moves away. */
  overlay: 2_000,

  /** All tracked application IPC completing and the result surviving two frames. */
  idle: 30_000,

  /**
   * A document mount. Covers the Rust read, decode, and CodeMirror state build,
   * so it is deliberately far longer than a UI reaction.
   */
  editor: 30_000,

  /**
   * A backend write landing on disk. Sized against the autosave contract in
   * `src-tauri/src/autosave.rs`: 1.5 s idle debounce with a 10 s maximum
   * latency, plus room for a loaded CI VM.
   */
  disk: 20_000,

  /**
   * Chunked or large-document work: the >100k-row CSV handoff and multi-MB
   * transformations, both of which stream results over several IPC messages.
   */
  heavy: 60_000,

} as const;

export const INTERVALS = {
  /** For cheap in-page reads (attributes, element presence). */
  fast: 100,
  /** For reads that cross a process boundary (disk, IPC). */
  slow: 250,
} as const;
