import fs from "node:fs";
import { INTERVALS, TIMEOUTS } from "../config/timeouts.js";

/**
 * Wait for a backend-owned filesystem signal without entering WebDriver's
 * serialized command queue. This remains responsive while the webview is
 * suspended or a WebDriver command is awaiting native window work.
 */
export function waitForProcessSignal(
  signalPath: string,
  message: string,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const interval = setInterval(() => {
      if (!fs.existsSync(signalPath)) return;
      clearInterval(interval);
      clearTimeout(timeout);
      resolve();
    }, INTERVALS.fast);
    const timeout = setTimeout(() => {
      clearInterval(interval);
      reject(new Error(message));
    }, TIMEOUTS.ui);
  });
}
