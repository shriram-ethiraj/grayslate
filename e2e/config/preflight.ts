import { execFileSync } from "node:child_process";
import fs from "node:fs";

/**
 * Environment preflight.
 *
 * The suite's most confusing failure mode is an environment gap that surfaces
 * as a product failure. Without a window manager, for example, the app's
 * maximize request is simply never honoured and the only symptom is
 * "the native window did not enter the maximized state" — which reads as an
 * application defect. Name the real cause up front instead.
 */

export interface PreflightProblem {
  what: string;
  why: string;
  fix: string;
  fatal: boolean;
}

function onPath(command: string): boolean {
  try {
    execFileSync("which", [command], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

function hasConfiguredLocale(localeName: string | undefined): boolean {
  if (!localeName) return false;
  try {
    const installed = execFileSync("locale", ["-a"], { encoding: "utf8" });
    const normalize = (value: string): string =>
      value.toLowerCase().replace(/[^a-z0-9]/g, "");
    const wanted = normalize(localeName);
    return installed.split(/\r?\n/).some((locale) => normalize(locale) === wanted);
  } catch {
    return false;
  }
}

export function checkEnvironment(appBinaryPath: string, driverPath?: string): PreflightProblem[] {
  const problems: PreflightProblem[] = [];

  if (!fs.existsSync(appBinaryPath)) {
    problems.push({
      what: `Packaged app missing at ${appBinaryPath}`,
      why: "The suite drives the real packaged binary; there is nothing to launch.",
      fix: "pnpm run e2e:build",
      fatal: true,
    });
  }

  if (!driverPath || !fs.existsSync(driverPath)) {
    problems.push({
      what: "tauri-driver not found",
      why: "WebDriver cannot reach the Tauri window without the bridge.",
      fix:
        "cargo install tauri-driver --version 2.0.6 --locked " +
        "(or set TAURI_DRIVER_PATH)",
      fatal: true,
    });
  }

  if (process.platform !== "linux") return problems;

  if (!onPath("WebKitWebDriver")) {
    problems.push({
      what: "WebKitWebDriver not found",
      why: "tauri-driver delegates to it to drive the WebKitGTK webview.",
      fix: "sudo apt-get install webkit2gtk-driver",
      fatal: true,
    });
  }

  const headless = !process.env.DISPLAY && !process.env.WAYLAND_DISPLAY;
  if (headless && !onPath("xvfb-run")) {
    problems.push({
      what: "No display and no Xvfb",
      why: "The app needs a display server to open its window.",
      fix: "sudo apt-get install xvfb, then use pnpm run e2e:ci",
      fatal: true,
    });
  }

  if (!onPath("openbox")) {
    problems.push({
      what: "No window manager (openbox) on PATH",
      why:
        "Window-state tests ask the compositor to maximize, minimize, and restore " +
        "the window. Without a window manager those transitions are ignored or " +
        "cannot be remapped, so the failure looks like an application defect.",
      fix: "sudo apt-get install openbox",
      // Non-fatal: everything except the window-state tests still runs, and
      // blocking the whole suite over one scenario helps nobody.
      fatal: false,
    });
  }

  if (!onPath("xprop")) {
    problems.push({
      what: "xprop not found",
      why:
        "The Linux launcher verifies that Openbox has claimed the X11 root " +
        "window before starting window-state scenarios.",
      fix: "sudo apt-get install x11-utils",
      fatal: true,
    });
  }

  if (!onPath("xclip") && !onPath("xsel")) {
    problems.push({
      what: "No native X11 clipboard reader (xclip or xsel)",
      why:
        "Clipboard scenarios verify Tauri's real OS clipboard out of process so " +
        "they do not mutate or refocus the app webview.",
      fix: "sudo apt-get install xclip",
      fatal: false,
    });
  }

  if (!hasConfiguredLocale(process.env.LC_ALL)) {
    problems.push({
      what: `Configured locale ${process.env.LC_ALL ?? "<unset>"} is unavailable`,
      why:
        "Sidebar collation and WebKit Intl defaults must use the same installed " +
        "locale on developer machines and CI workers.",
      fix: "sudo locale-gen en_US.UTF-8",
      fatal: true,
    });
  }

  return problems;
}

/** Print preflight findings; throw only when the suite genuinely cannot run. */
export function reportEnvironment(appBinaryPath: string, driverPath?: string): void {
  const problems = checkEnvironment(appBinaryPath, driverPath);
  if (problems.length === 0) return;

  const describe = (problem: PreflightProblem): string =>
    `  - ${problem.what}\n      ${problem.why}\n      fix: ${problem.fix}`;

  const fatal = problems.filter((problem) => problem.fatal);
  const warnings = problems.filter((problem) => !problem.fatal);

  if (warnings.length > 0) {
    console.warn(
      `\n[e2e preflight] ${warnings.length} environment gap(s); some scenarios will fail:\n` +
        warnings.map(describe).join("\n") +
        "\n",
    );
  }

  if (fatal.length > 0) {
    throw new Error(
      `E2E environment is not ready:\n${fatal.map(describe).join("\n")}\n`,
    );
  }
}
