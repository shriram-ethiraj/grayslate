import { browser } from "@wdio/globals";
import { TIMEOUTS } from "../config/timeouts.js";
import { waitUntilReady, type ReadyOptions } from "../pages/editor.js";
import { waitFor } from "./wait.js";

/**
 * Restart the packaged application inside the current spec sandbox.
 *
 * `reloadSession()` closes the current Tauri/WebDriver session and launches a
 * new process with the same requested capabilities. The worker HOME/XDG paths
 * do not change, so SQLite settings and files survive while all webview memory
 * is genuinely rebuilt.
 */
export async function restartApp(
  expectedEditor: ReadyOptions = {},
): Promise<void> {
  await browser.reloadSession();

  // `reloadSession()` resolves once tauri-driver has accepted the replacement
  // session. The new desktop process may still be starting and may not have
  // registered its `main` webview yet, especially on Linux/WebKit. Switching
  // immediately races that registration and permanently targets the closed
  // session's window handle.
  let observedHandles: string[] = [];
  await waitFor(
    async () => {
      try {
        observedHandles = await browser.getWindowHandles();
        return observedHandles.length > 0;
      } catch {
        observedHandles = [];
        return false;
      }
    },
    {
      message: () =>
        `The restarted application never registered a window. ` +
        `Last handles: ${JSON.stringify(observedHandles)}`,
      timeoutMs: TIMEOUTS.heavy,
    },
  );

  // The Tauri service's label lookup uses the guest direct-eval bridge, which
  // is not available until after a window becomes current. The restarted app
  // has exactly one initial window, so select the fresh WebDriver handle first
  // and let the bridge initialize from there.
  await browser.switchToWindow(observedHandles[0]!);
  await browser.setWindowSize(1440, 900).catch(() => {
    // Layout-sensitive scenarios carry their own assertion if resize support
    // is unavailable; restart behavior itself does not depend on this.
  });
  await waitUntilReady({
    ...expectedEditor,
    timeoutMs: expectedEditor.timeoutMs ?? TIMEOUTS.heavy,
  });
}
