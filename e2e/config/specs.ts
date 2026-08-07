import fs from "node:fs";
import path from "node:path";

/**
 * Spec discovery and ordering.
 *
 * The suite previously listed its 20 spec files as a hand-maintained array in
 * `wdio.conf.ts`. A new spec file was then silently never run — no error, no
 * warning, just missing coverage. Discovery removes that failure mode entirely:
 * a file on disk is a file that runs.
 *
 * Ordering is by tier, then lexicographic path. Because every spec file gets
 * its own packaged-app session (see `sandboxReset` in `wdio.base.ts`), order
 * between files carries no state and is purely about reading a failing CI log
 * top to bottom.
 */

export type SuiteName = "functional" | "visual" | "security";

const SPECS_ROOT = path.resolve(process.cwd(), "e2e", "specs");

/** Later tiers run last. Security stays isolated at the end, as designed. */
const TIER_ORDER: Record<SuiteName, number> = {
  functional: 0,
  visual: 1,
  security: 2,
};

function suiteOf(relativePath: string): SuiteName {
  const [head] = relativePath.split(path.sep);
  if (head === "security") return "security";
  if (head === "visual") return "visual";
  return "functional";
}

function walk(directory: string): string[] {
  if (!fs.existsSync(directory)) return [];
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) return walk(absolute);
    return entry.isFile() && entry.name.endsWith(".e2e.ts") ? [absolute] : [];
  });
}

/** Every spec on disk, grouped and ordered. Never hand-maintained. */
export function discoverSpecs(suites?: SuiteName[]): string[] {
  const wanted = new Set<SuiteName>(suites ?? ["functional", "visual", "security"]);

  return walk(SPECS_ROOT)
    .map((absolute) => {
      const relative = path.relative(SPECS_ROOT, absolute);
      return { absolute, relative, suite: suiteOf(relative) };
    })
    .filter((spec) => wanted.has(spec.suite))
    .sort((a, b) => {
      const tier = TIER_ORDER[a.suite] - TIER_ORDER[b.suite];
      return tier !== 0 ? tier : a.relative.localeCompare(b.relative);
    })
    .map((spec) => spec.absolute);
}

/** Suite definitions for `wdio run … --suite <name>`. */
export function suiteMap(): Record<SuiteName, string[]> {
  return {
    functional: discoverSpecs(["functional"]),
    visual: discoverSpecs(["visual"]),
    security: discoverSpecs(["security"]),
  };
}
