import fs from "node:fs";
import path from "node:path";
import { browser } from "@wdio/globals";
import type { TauriCapabilities } from "@wdio/tauri-service";
import { waitForEditorReady } from "./helpers/app.js";
import {
  artifactRoot,
  configureSandboxEnvironment,
  resetE2eRunDirectory,
  workerId,
} from "./helpers/sandbox.js";

// The WDIO worker inherits the sandbox HOME set by the launcher process. Keep
// the original home available so a Cargo-installed tauri-driver can still be
// located when the worker loads this config again.
if (!process.env.GRAYSLATE_E2E_HOST_HOME && process.env.HOME) {
  process.env.GRAYSLATE_E2E_HOST_HOME = process.env.HOME;
}
const hostHome = process.env.GRAYSLATE_E2E_HOST_HOME ?? process.env.HOME;
const tauriDriverCandidates =
  process.platform === "win32"
    ? [
        path.join(process.env.USERPROFILE ?? "", ".cargo", "bin", "tauri-driver.exe"),
        "C:\\Users\\runneradmin\\.cargo\\bin\\tauri-driver.exe",
      ]
    : [
        path.join(hostHome ?? "", ".cargo", "bin", "tauri-driver"),
        "/usr/local/bin/tauri-driver",
      ];
const tauriDriverPath =
  process.env.TAURI_DRIVER_PATH ??
  tauriDriverCandidates.find((candidate) => fs.existsSync(candidate));
const isWorkerProcess = process.env.WDIO_WORKER_ID !== undefined;
if (!isWorkerProcess) {
  resetE2eRunDirectory();
}
// The launcher starts tauri-driver with this environment. Each serial worker
// then clears the same runtime before its new packaged-app session starts.
configureSandboxEnvironment();

const appBinaryName = process.platform === "win32" ? "Grayslate.exe" : "Grayslate";
const appBinaryPath = path.resolve(
  process.cwd(),
  "target/release",
  appBinaryName,
);
const artifactDirectory = artifactRoot;
let mainWindowPinned = false;
const tauriCapabilities: TauriCapabilities = {
  browserName: "tauri",
  "tauri:options": {
    application: appBinaryPath,
  },
};

if (isWorkerProcess) {
  fs.mkdirSync(artifactDirectory, { recursive: true });
}

function artifactStem(title: string): string {
  return title.replace(/[^a-zA-Z0-9_-]+/g, "-").slice(0, 100) || "e2e-test";
}

if (!fs.existsSync(appBinaryPath)) {
  throw new Error(
    `The E2E app binary was not found at ${appBinaryPath}. Run ` +
      "pnpm run e2e:build before pnpm run e2e:test.",
  );
}

export const config: WebdriverIO.Config = {
  runner: "local",
  rootDir: process.cwd(),
  // A flat list gives every spec file a fresh packaged-app/WebKit session.
  // maxInstances: 1 keeps those sessions strictly serial in this one CI job.
  specs: [
    "./e2e/specs/00-selectors-smoke.e2e.ts",
    "./e2e/specs/01-first-run.e2e.ts",
    "./e2e/specs/02-external-files.e2e.ts",
    "./e2e/specs/03-language-detection.e2e.ts",
    "./e2e/specs/04-editor-core.e2e.ts",
    "./e2e/specs/05-formatting-indent.e2e.ts",
    "./e2e/specs/06-transformations.e2e.ts",
    "./e2e/specs/07-sidebar.e2e.ts",
    "./e2e/specs/08-appearance.e2e.ts",
    "./e2e/specs/09-markdown.e2e.ts",
    "./e2e/specs/10-csv.e2e.ts",
    "./e2e/specs/11-keyboard-shortcuts.e2e.ts",
    "./e2e/specs/11-app-shell.e2e.ts",
    "./e2e/specs/security/document-authorization.e2e.ts",
    "./e2e/specs/security/ipc-capabilities.e2e.ts",
    "./e2e/specs/security/webview-security.e2e.ts",
  ],
  maxInstances: 1,
  // Native action commands are verbose at `info`; warnings and failures still
  // remain visible while keeping local/CI output readable.
  logLevel: "warn",
  waitforTimeout: 15_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 1,
  framework: "mocha",
  reporters: ["spec"],
  services: [
    ["@wdio/tauri-service", {
      appBinaryPath,
      driverProvider: "external",
      autoInstallTauriDriver: false,
      tauriDriverPath,
      startTimeout: 60_000,
      commandTimeout: 30_000,
    }],
  ],
  capabilities: [tauriCapabilities],
  mochaOpts: {
    ui: "bdd",
    // A native Tauri session includes driver startup, real Rust detection,
    // and debounced autosave. Keep this separate from the individual waits in
    // the spec so a slow GitHub Actions VM does not abort the whole scenario.
    timeout: 120_000,
  },
  beforeSession: function (_config, _capabilities, specs, cid) {
    fs.mkdirSync(artifactDirectory, { recursive: true });
    fs.writeFileSync(
      path.join(artifactDirectory, "worker.json"),
      `${JSON.stringify({ cid, workerId, specs }, null, 2)}\n`,
      "utf8",
    );
  },
  beforeSuite: async function () {
    // Pin the known production window label once per worker and wait for the
    // initial CodeMirror session before the spec starts interacting with it.
    if (!mainWindowPinned) {
      await browser.tauri.switchWindow("main");
      mainWindowPinned = true;
    }
    await waitForEditorReady();
  },
  afterTest: async function (test, _context, result) {
    if (result.passed) {
      return;
    }

    const stem = artifactStem(test.title);
    fs.writeFileSync(
      path.join(artifactDirectory, `${stem}.json`),
      `${JSON.stringify({ workerId, title: test.title }, null, 2)}\n`,
      "utf8",
    );
    try {
      await browser.saveScreenshot(path.join(artifactDirectory, `${stem}.png`));
    } catch {
      // Preserve the original test failure if the driver has already exited.
    }

    try {
      fs.writeFileSync(
        path.join(artifactDirectory, `${stem}.html`),
        await browser.getPageSource(),
        "utf8",
      );
    } catch {
      // Preserve the original test failure if page source is unavailable.
    }
  },
};
