import fs from "node:fs";
import path from "node:path";

export const e2eRunRoot = path.resolve(process.cwd(), ".e2e-tmp");
export const workersRoot = path.join(e2eRunRoot, "workers");
export const runtimeRoot = path.join(e2eRunRoot, "runtime");
export const workerId = (process.env.WDIO_WORKER_ID ?? "launcher")
  .replace(/[^a-zA-Z0-9_-]+/g, "-");
// tauri-driver is started once by the WDIO launcher when maxInstances is 1.
// Every serial worker resets this shared runtime before tauri-driver launches
// that worker's fresh app session. Artifacts remain worker-specific below.
export const sandboxRoot = runtimeRoot;
export const artifactRoot = path.join(workersRoot, workerId, "artifacts");
export const homeDirectory = path.join(sandboxRoot, "home");
export const configDirectory = path.join(sandboxRoot, "config");
export const dataDirectory = path.join(sandboxRoot, "data");
export const cacheDirectory = path.join(sandboxRoot, "cache");
export const stateDirectory = path.join(sandboxRoot, "state");
export const notesRoot = path.join(homeDirectory, "Documents", "Grayslate");

/** Remove stale E2E output once, before WDIO starts any worker process. */
export function resetE2eRunDirectory(): void {
  fs.rmSync(e2eRunRoot, { recursive: true, force: true });
  fs.mkdirSync(workersRoot, { recursive: true });
}

/**
 * A small, deterministic Rust document that exercises both the detector and
 * the naming pipeline. The final newline is intentional: the test types it
 * through the editor one line at a time and verifies the exact saved bytes.
 * Declarations keep balanced braces on one line so CodeMirror auto-indent does
 * not change the bytes while the test is typing them.
 */
export const rustFixture = `use std::collections::HashMap;
#[derive(Debug, Clone)]
pub struct Config { pub name: String }
pub fn process(config: &Config) -> Result<(), String> { println!("Processing: {}", config.name); Ok(()) }
`;

/**
 * Reset all user-facing state used by the desktop process. This keeps a local
 * run repeatable without touching the developer's real Grayslate data.
 */
export function configureSandboxEnvironment(): void {
  fs.rmSync(sandboxRoot, { recursive: true, force: true });
  fs.mkdirSync(homeDirectory, { recursive: true });
  fs.mkdirSync(configDirectory, { recursive: true });
  fs.mkdirSync(dataDirectory, { recursive: true });
  fs.mkdirSync(cacheDirectory, { recursive: true });
  fs.mkdirSync(stateDirectory, { recursive: true });
  fs.mkdirSync(notesRoot, { recursive: true });

  // Tauri's BaseDirectory::Document follows the freedesktop user-dirs file.
  // Point it at the sandbox's Documents directory instead of the host user's.
  fs.writeFileSync(
    path.join(configDirectory, "user-dirs.dirs"),
    'XDG_DOCUMENTS_DIR="$HOME/Documents"\n',
    "utf8",
  );

  Object.assign(process.env, {
    HOME: homeDirectory,
    XDG_CONFIG_HOME: configDirectory,
    XDG_DATA_HOME: dataDirectory,
    XDG_CACHE_HOME: cacheDirectory,
    XDG_STATE_HOME: stateDirectory,
  });
}
