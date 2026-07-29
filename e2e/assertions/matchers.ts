import fs from "node:fs";
import path from "node:path";
import { expect } from "@wdio/globals";
import { INTERVALS, TIMEOUTS } from "../config/timeouts.js";

/**
 * Shared assertions for the facts this suite actually cares about: exact bytes
 * on disk, exact directory contents, and negatives that are genuinely settled
 * rather than merely slept through.
 */

/** Assert a file's bytes exactly, with a readable diff on failure. */
export function expectFileBytes(filePath: string, expected: Buffer | string): void {
  const expectedBytes = Buffer.isBuffer(expected) ? expected : Buffer.from(expected, "utf8");
  const actual = fs.readFileSync(filePath);
  if (actual.equals(expectedBytes)) return;

  throw new Error(
    `Bytes differ for ${filePath}\n` +
      `  expected: ${JSON.stringify(expectedBytes.toString("utf8"))} (${expectedBytes.length} bytes)\n` +
      `  actual:   ${JSON.stringify(actual.toString("utf8"))} (${actual.length} bytes)\n` +
      `  expected hex: ${expectedBytes.toString("hex")}\n` +
      `  actual hex:   ${actual.toString("hex")}`,
  );
}

/** The regular (non-directory) file names in a directory, sorted. */
export function directoryInventory(directory: string): string[] {
  if (!fs.existsSync(directory)) return [];
  return fs
    .readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name)
    .sort();
}

/**
 * Assert a directory contains exactly these files.
 *
 * The strongest "no stray file" contract available: it catches an autosave that
 * forks a second slate, a Save-As that leaves the original behind, and a rename
 * that copies instead of moving.
 */
export function expectDirectoryInventory(directory: string, expected: string[]): void {
  expect(directoryInventory(directory)).toEqual([...expected].sort());
}

/** Absolute paths of the regular files in a directory, sorted. */
export function directoryPaths(directory: string): string[] {
  return directoryInventory(directory).map((name) => path.join(directory, name));
}

/**
 * Assert an operation added no files beyond the ones named.
 *
 * The sandbox is wiped per spec *file*, not per test, so sibling scenarios in
 * the same file legitimately leave their own fixtures behind. An absolute
 * inventory assertion would therefore fail for a reason that has nothing to do
 * with the behavior under test. What actually matters is the delta: did this
 * operation fork a second document, or leave a stray copy?
 */
export function expectNoNewFiles(
  directory: string,
  baseline: string[],
  allowed: string[] = [],
): void {
  const now = directoryInventory(directory);
  const expected = new Set([...baseline, ...allowed]);
  const unexpected = now.filter((name) => !expected.has(name));

  if (unexpected.length === 0) return;
  throw new Error(
    `Unexpected file(s) appeared in ${directory}: ${unexpected.join(", ")}\n` +
      `  before: ${baseline.join(", ") || "(empty)"}\n` +
      `  after:  ${now.join(", ")}`,
  );
}

export interface SettledAbsentOptions {
  /**
   * A positive wait proving the system advanced far enough that a violation
   * would already have surfaced — for example, "the save that *should* happen
   * has landed on disk". Without this the invariant is sampled against a system
   * that simply has not started working yet, and the assertion passes vacuously.
   */
  precondition: () => Promise<void>;
  /** Sampled repeatedly; must hold for the entire quiet window. */
  invariant: () => boolean | Promise<boolean>;
  /** What must stay true, phrased as the guarantee under test. */
  message: string;
  quietForMs?: number;
  intervalMs?: number;
}

/**
 * Assert that something stays absent, without sleeping.
 *
 * This replaces the `browser.pause(2_500)`-then-assert pattern the suite used
 * for its most valuable negatives (autosave must not touch a local file; a
 * repeated Save must not fork a second slate; a blocked navigation must not
 * proceed). A fixed sleep makes those assertions *weaker* the slower the
 * machine gets: on a loaded CI VM the violation simply had not happened yet, so
 * the test passed for the wrong reason.
 *
 * The contract here is different: advance the system to a known point, then
 * require the invariant to hold across every sample of a quiet window.
 */
export async function expectSettledAbsent(options: SettledAbsentOptions): Promise<void> {
  await options.precondition();

  const quietForMs = options.quietForMs ?? TIMEOUTS.quiescence;
  const intervalMs = options.intervalMs ?? INTERVALS.slow;
  const deadline = Date.now() + quietForMs;
  let samples = 0;

  while (Date.now() < deadline) {
    if (!(await options.invariant())) {
      throw new Error(
        `Invariant broke after ${samples} sample(s) in the quiet window: ${options.message}`,
      );
    }
    samples += 1;
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }

  if (samples === 0) {
    throw new Error(
      `Quiet window elapsed without sampling the invariant: ${options.message}`,
    );
  }
}
