import fs from "node:fs";
import { $, browser } from "@wdio/globals";
import { INTERVALS, TIMEOUTS } from "../config/timeouts.js";
import { isTransientWebDriverError } from "./interact.js";

/**
 * Wait primitives that make the compliant path the easy path.
 *
 * Every wait in this suite must carry an explicit timeout and a failure
 * message. Bare `browser.waitUntil(fn)` inherits the global 15 s ceiling and
 * fails with a generic driver message that says nothing about what the test
 * was actually waiting for — the suite had ~25 of those. These wrappers make
 * both mandatory at the type level, and `e2e/scripts/lint-conventions.mjs`
 * rejects raw `waitUntil` calls that omit them.
 */
export interface WaitOptions {
  /**
   * What the caller was waiting for, phrased as the expected end state.
   *
   * Pass a function whenever the message quotes something the poll observes.
   * A template string is evaluated where it is written — before a single poll
   * has run — so `` `... Last observed: ${observed}` `` reports the variable's
   * *initial* value forever. That bug made every text mismatch in this suite
   * report `Last observed: ""`, which read as "the editor is empty" and sent a
   * previous investigation after a non-existent CodeMirror defect.
   */
  message: string | (() => string);
  timeoutMs?: number;
  intervalMs?: number;
}

/** Identifies an unmet condition without conflating it with a driver failure. */
export class WaitTimeoutError extends Error {}

/** Poll `predicate` until it is true, or fail with `message`. */
export async function waitFor(
  predicate: () => boolean | Promise<boolean>,
  options: WaitOptions,
): Promise<void> {
  try {
    await browser.waitUntil(async () => predicate(), {
      timeout: options.timeoutMs ?? TIMEOUTS.ui,
      interval: options.intervalMs ?? INTERVALS.fast,
      // Placeholder: the real message is raised below, after the final poll, so
      // a lazy message reports what was actually last seen.
      timeoutMsg: "waitFor timed out",
    });
  } catch (error) {
    const message = typeof options.message === "function" ? options.message() : options.message;
    // Preserve a driver-level failure (stale session, lost window) rather than
    // relabelling it as the caller's expectation, which would hide it.
    const detail = error instanceof Error ? error.message : String(error);
    if (!detail.includes("waitFor timed out")) {
      throw new Error(`${message} (driver reported: ${detail})`);
    }
    throw new WaitTimeoutError(message);
  }
}

/**
 * Wait for an element's attribute to equal `expected`.
 *
 * The dominant assertion shape in this suite — `data-eol`, `data-encoding`,
 * `data-language-mode`, `aria-disabled`, `aria-pressed` — previously
 * reimplemented per call site, often as an un-waited point read taken
 * immediately after a click.
 */
export async function waitForAttribute(
  testId: string,
  attribute: string,
  expected: string | null,
  options?: Partial<WaitOptions>,
): Promise<void> {
  await waitFor(
    async () => {
      try {
        const element = await $(`[data-testid='${testId}']`);
        if (!(await element.isExisting())) return false;
        return (await element.getAttribute(attribute)) === expected;
      } catch (error) {
        // Svelte can replace a control between the existence and attribute
        // reads. Reacquire it on the next poll, but never turn a lost session
        // or another driver failure into an ordinary timeout.
        if (isTransientWebDriverError(error)) return false;
        throw error;
      }
    },
    {
      message:
        options?.message ??
        `Element '${testId}' never reported ${attribute}='${expected}'.`,
      timeoutMs: options?.timeoutMs,
      intervalMs: options?.intervalMs,
    },
  );
}

/**
 * Wait until a file on disk satisfies `predicate` (defaults to "exists").
 *
 * Reads through Node, not the webview, so it observes the real write the Rust
 * side performed rather than any frontend belief about it.
 */
export async function waitForFile(
  filePath: string,
  predicate: (content: string) => boolean = () => true,
  options?: Partial<WaitOptions>,
): Promise<void> {
  await waitFor(
    () => {
      try {
        return predicate(fs.readFileSync(filePath, "utf8"));
      } catch {
        // ENOENT while the write is still pending. Keep polling; the timeout
        // is the failure signal, not this read.
        return false;
      }
    },
    {
      message: options?.message ?? `File condition never met for ${filePath}`,
      timeoutMs: options?.timeoutMs ?? TIMEOUTS.disk,
      intervalMs: options?.intervalMs ?? INTERVALS.slow,
    },
  );
}

/** The raw-bytes counterpart of `waitForFile`, for encoding and EOL assertions. */
export async function waitForFileBytes(
  filePath: string,
  predicate: (bytes: Buffer) => boolean,
  options?: Partial<WaitOptions>,
): Promise<void> {
  await waitFor(
    () => {
      try {
        return predicate(fs.readFileSync(filePath));
      } catch {
        return false;
      }
    },
    {
      message: options?.message ?? `File bytes never matched for ${filePath}`,
      timeoutMs: options?.timeoutMs ?? TIMEOUTS.disk,
      intervalMs: options?.intervalMs ?? INTERVALS.slow,
    },
  );
}

/**
 * Poll a disk condition without going through WebDriver.
 *
 * Needed only after the window has been destroyed (the close-with-save flows in
 * the lifecycle specs), where `browser.waitUntil` has no session to run in.
 */
export async function waitForFileWithoutWebDriver(
  filePath: string,
  predicate: (content: string) => boolean,
  options?: Partial<WaitOptions>,
): Promise<void> {
  const timeoutMs = options?.timeoutMs ?? TIMEOUTS.disk;
  const intervalMs = options?.intervalMs ?? INTERVALS.slow;
  const deadline = Date.now() + timeoutMs;

  while (Date.now() < deadline) {
    try {
      if (predicate(fs.readFileSync(filePath, "utf8"))) return;
    } catch {
      // The file may not exist yet; the deadline below is the failure signal.
    }
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }

  const message = typeof options?.message === "function" ? options.message() : options?.message;
  throw new Error(
    message ?? `File condition never met for ${filePath} after the window closed.`,
  );
}
