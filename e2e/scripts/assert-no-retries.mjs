#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const workersRoot = path.resolve(process.cwd(), ".e2e-tmp", "workers");

function walk(directory) {
  if (!fs.existsSync(directory)) return [];
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) return walk(absolute);
    return entry.isFile() && entry.name === "retries.json" ? [absolute] : [];
  });
}

const reports = walk(workersRoot);
if (reports.length === 0) {
  console.error("E2E retry audit found no worker retry reports.");
  process.exit(1);
}

const unstable = reports.flatMap((reportPath) => {
  const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
  return report.totalRetries > 0
    ? [{ reportPath: path.relative(process.cwd(), reportPath), ...report }]
    : [];
});

if (unstable.length > 0) {
  console.error(
    "E2E stability audit failed: passing scenarios required transient interaction retries.\n" +
      `${JSON.stringify(unstable, null, 2)}\n`,
  );
  process.exit(1);
}

console.log(`E2E stability audit: ${reports.length} worker(s), zero interaction retries.`);
