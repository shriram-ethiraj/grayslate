import fs from "node:fs";
import path from "node:path";
import { expect } from "@wdio/globals";

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
