import fs from "node:fs";
import path from "node:path";

export const FORBIDDEN_FRONTEND_MARKERS = [
  "__grayslateE2E",
  "grayslate-e2e-determinism",
];

function walkFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) return walkFiles(absolute);
    return entry.isFile() ? [absolute] : [];
  });
}

/** Scan the uncompressed frontend output produced immediately before Tauri embeds it. */
export function findForbiddenFrontendMarkers(frontendDirectory) {
  if (!fs.existsSync(frontendDirectory)) {
    throw new Error(`Frontend build output is missing at ${frontendDirectory}.`);
  }

  const found = new Set();
  for (const file of walkFiles(frontendDirectory)) {
    const contents = fs.readFileSync(file);
    for (const marker of FORBIDDEN_FRONTEND_MARKERS) {
      if (contents.includes(marker)) found.add(marker);
    }
  }
  return [...found];
}
