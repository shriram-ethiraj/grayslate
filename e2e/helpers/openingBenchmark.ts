import fs from "node:fs";
import path from "node:path";
import { homeDirectory } from "./sandbox.js";

/** Linux resident memory for the isolated application and its child processes. */
export function applicationRssKiB(): number | null {
  if (process.platform !== "linux") return null;
  const processes: { pid: number; parent: number; rss: number }[] = [];
  let root: number | undefined;
  for (const entry of fs.readdirSync("/proc")) {
    if (!/^\d+$/.test(entry)) continue;
    try {
      const status = fs.readFileSync(`/proc/${entry}/status`, "utf8");
      const pid = Number(entry);
      processes.push({ pid, parent: Number(status.match(/^PPid:\s+(\d+)/m)?.[1]), rss: Number(status.match(/^VmRSS:\s+(\d+)/m)?.[1] ?? 0) });
      if (/^Name:\s+Grayslate$/m.test(status)) {
        const environment = fs.readFileSync(`/proc/${entry}/environ`, "utf8").split("\0");
        if (environment.includes(`HOME=${homeDirectory}`)) root = pid;
      }
    } catch { /* A process may exit between directory enumeration and read. */ }
  }
  if (!root) return null;
  const descendants = new Set([root]);
  for (let changed = true; changed;) {
    changed = false;
    for (const process of processes) {
      if (descendants.has(process.parent) && !descendants.has(process.pid)) {
        descendants.add(process.pid);
        changed = true;
      }
    }
  }
  return processes.filter((process) => descendants.has(process.pid)).reduce((sum, process) => sum + process.rss, 0);
}

export function recordOpeningBenchmark(result: object): void {
  const destination = path.resolve(".e2e-tmp", "opening-benchmark.jsonl");
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.appendFileSync(destination, `${JSON.stringify(result)}\n`);
}
