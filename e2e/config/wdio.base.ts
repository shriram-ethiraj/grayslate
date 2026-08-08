import fs from "node:fs";
import path from "node:path";
import { browser } from "@wdio/globals";
import type { TauriCapabilities } from "@wdio/tauri-service";
import { clearRetryLog, readRetryLog } from "../driver/interact.js";
import { waitForBootstrap, waitUntilReady } from "../pages/editor.js";
import {
  artifactRoot,
  configureSandboxEnvironment,
  resetE2eRunDirectory,
  workerId,
} from "../helpers/sandbox.js";
import { TIMEOUTS } from "./timeouts.js";
import { reportEnvironment } from "./preflight.js";
import { discoverSpecs, suiteMap, type SuiteName } from "./specs.js";

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

/**
 * ISOLATION CONTRACT — read before changing anything below.
 *
 * The launcher starts `tauri-driver` with this environment; each serial worker
 * then wipes the same runtime tree before its new packaged-app session starts.
 * That wipe is what guarantees a spec file never sees another spec file's
 * files, and it is what makes assertions like `expect(notesRoot).toEqual([])`
 * meaningful.
 *
 * It holds only while exactly one worker runs at a time and each worker loads
 * one spec file. Raising `maxInstances`, adding `specFileRetries`, or grouping
 * spec paths into nested arrays silently breaks it — files from two specs would
 * collide in one sandbox and the inventory assertions would start failing for
 * reasons unrelated to the product. `e2e/specs/meta/harness-contract.e2e.ts`
 * asserts these invariants so the breakage is loud rather than mysterious.
 */
configureSandboxEnvironment();

const appBinaryName = process.platform === "win32" ? "Grayslate.exe" : "Grayslate";
const appBinaryPath = path.resolve(process.cwd(), "target/release", appBinaryName);
const artifactDirectory = artifactRoot;

// Name environment gaps up front. A missing window manager otherwise surfaces
// much later as "the native window did not enter the maximized state", which
// reads as an application defect.
reportEnvironment(appBinaryPath, tauriDriverPath);

if (isWorkerProcess) {
  fs.mkdirSync(artifactDirectory, { recursive: true });
}

const runRetries: {
  title: string;
  retries: { selector: string; attempt: number; reason: string }[];
}[] = [];
let workerRuntimeReady = false;

const tauriCapabilities: TauriCapabilities = {
  browserName: "tauri",
  "tauri:options": { application: appBinaryPath },
};

function artifactStem(title: string): string {
  return title.replace(/[^a-zA-Z0-9_-]+/g, "-").slice(0, 100) || "e2e-test";
}

/**
 * Build a WDIO config for a set of suites.
 *
 * `specs` is discovered from disk rather than hand-listed, so adding a spec
 * file is sufficient to make it run.
 */
export function makeConfig(suites?: SuiteName[]): WebdriverIO.Config {
  const specs = discoverSpecs(suites);
  if (specs.length === 0) {
    throw new Error(
      `No spec files were discovered for suites: ${(suites ?? ["all"]).join(", ")}.`,
    );
  }

  return {
    runner: "local",
    rootDir: process.cwd(),
    // A flat list gives every spec file a fresh packaged-app/WebKit session.
    // maxInstances: 1 keeps those sessions strictly serial. See the isolation
    // contract above before changing either.
    specs,
    suites: suiteMap(),
    maxInstances: 1,
    // The Tauri service's optional direct-eval focus probe is tied to the
    // original process and reports a warning before every command after a
    // deliberate `reloadSession()`. Keep dependency internals at `error`;
    // assertion/protocol failures, our explicit retry warnings, screenshots,
    // JUnit output, and the preflight report remain visible.
    logLevel: "error",
    waitforTimeout: TIMEOUTS.ui,
    connectionRetryTimeout: 120_000,
    connectionRetryCount: 1,
    framework: "mocha",
    reporters: [
      "spec",
      // A machine-readable artifact so CI can surface which scenario failed
      // without scraping the console log.
      [
        "junit",
        {
          outputDir: artifactDirectory,
          outputFileFormat: (options: { cid: string }) => `results-${options.cid}.xml`,
        },
      ],
    ],
    services: [
      [
        "@wdio/tauri-service",
        {
          appBinaryPath,
          driverProvider: "external",
          autoInstallTauriDriver: false,
          tauriDriverPath,
          startTimeout: 60_000,
          commandTimeout: 30_000,
        },
      ],
    ],
    capabilities: [tauriCapabilities],
    mochaOpts: {
      ui: "bdd",
      // Every spec file owns an isolated app process. Once one scenario proves
      // that process is broken, continuing only creates dependent noise.
      bail: true,
      // A native Tauri session includes driver startup, real Rust detection,
      // and debounced autosave. Keep this separate from the individual waits in
      // the spec so a slow GitHub Actions VM does not abort the whole scenario.
      timeout: 120_000,
    },
    beforeSession: function (_config, _capabilities, sessionSpecs, cid) {
      fs.mkdirSync(artifactDirectory, { recursive: true });
      fs.writeFileSync(
        path.join(artifactDirectory, "worker.json"),
        `${JSON.stringify({ cid, workerId, specs: sessionSpecs }, null, 2)}\n`,
        "utf8",
      );
    },
    beforeTest: async function () {
      clearRetryLog();
      if (workerRuntimeReady) return;

      // Run startup as part of the first Mocha scenario so a failure is a real
      // test failure and `bail` stops the isolated worker immediately. WDIO's
      // top-level `before` hook only logs failures and then continues.
      const handles = await browser.getWindowHandles();
      if (handles.length !== 1) {
        throw new Error(
          `Expected one initial Grayslate window, received ${JSON.stringify(handles)}.`,
        );
      }
      await browser.switchToWindow(handles[0]!);
      // Give the app a predictable, generous viewport. At the default size the
      // sidebar header's controls overlap the search input.
      await browser.setWindowSize(1440, 900).catch(() => {
        // Layout-sensitive scenarios carry their own assertion if resize
        // support is unavailable.
      });
      await waitForBootstrap();
      await waitUntilReady();
      workerRuntimeReady = true;
    },
    afterTest: async function (test, _context, result) {
      const stem = artifactStem(test.title);
      const retries = readRetryLog().map((entry) => ({ ...entry }));
      if (retries.length > 0) {
        runRetries.push({ title: test.title, retries });
      }

      if (!result.passed || retries.length > 0) {
        fs.writeFileSync(
          path.join(artifactDirectory, `${stem}.json`),
          `${JSON.stringify(
            {
              workerId,
              title: test.title,
              passed: result.passed,
              retries,
            },
            null,
            2,
          )}\n`,
          "utf8",
        );
      }

      if (result.passed) return;

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
    after: function () {
      fs.writeFileSync(
        path.join(artifactDirectory, "retries.json"),
        `${JSON.stringify(
          {
            workerId,
            totalRetries: runRetries.reduce(
              (total, test) => total + test.retries.length,
              0,
            ),
            tests: runRetries,
          },
          null,
          2,
        )}\n`,
        "utf8",
      );
    },
  };
}
