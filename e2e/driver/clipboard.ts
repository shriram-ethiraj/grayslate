import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { INTERVALS, TIMEOUTS } from "../config/timeouts.js";

const execFileAsync = promisify(execFile);

interface ClipboardCommand {
  executable: string;
  args: string[];
}

function clipboardCommands(): ClipboardCommand[] {
  if (process.platform === "darwin") {
    return [{ executable: "pbpaste", args: [] }];
  }
  if (process.platform === "win32") {
    return [{
      executable: "powershell.exe",
      args: [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "[Console]::Out.Write((Get-Clipboard -Raw))",
      ],
    }];
  }
  return [
    { executable: "xclip", args: ["-selection", "clipboard", "-out"] },
    { executable: "xsel", args: ["--clipboard", "--output"] },
  ];
}

async function readClipboardText(): Promise<string> {
  const failures: string[] = [];
  for (const command of clipboardCommands()) {
    try {
      const { stdout } = await execFileAsync(command.executable, command.args, {
        encoding: "utf8",
        timeout: 2_000,
        maxBuffer: 32 * 1024 * 1024,
      });
      return stdout;
    } catch (error) {
      failures.push(`${command.executable}: ${String(error)}`);
    }
  }
  throw new Error(`No native clipboard reader succeeded (${failures.join("; ")}).`);
}

/**
 * Wait until the system clipboard holds exactly `expected`.
 *
 * The app writes through Tauri's native clipboard plugin. Reading it with the
 * host's native tool verifies that OS boundary directly and, unlike a pasted
 * textarea probe, does not mutate the app DOM or steal editor focus.
 */
export async function waitForClipboardText(expected: string): Promise<void> {
  const deadline = Date.now() + TIMEOUTS.ui;
  let actual = "";
  let readError = "";

  while (Date.now() < deadline) {
    try {
      actual = await readClipboardText();
      readError = "";
      if (actual === expected) return;
    } catch (error) {
      readError = error instanceof Error ? error.message : String(error);
    }
    await new Promise((resolve) => setTimeout(resolve, INTERVALS.fast));
  }

  throw new Error(
    `The clipboard never held the expected text.\n` +
      `  expected: ${JSON.stringify(expected)}\n` +
      `  actual:   ${JSON.stringify(actual)}\n` +
      (readError ? `  reader:   ${readError}` : ""),
  );
}
