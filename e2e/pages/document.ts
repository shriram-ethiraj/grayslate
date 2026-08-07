import fs from "node:fs";
import path from "node:path";
import { TIMEOUTS } from "../config/timeouts.js";
import { waitFor, waitForAppStable } from "../driver/wait.js";
import * as titleBar from "./titleBar.js";

export interface CommittedDocumentOptions {
  content: string;
  directory: string;
  timeoutMs?: number;
}

/**
 * Wait for an autosaved document to acquire its final application identity.
 *
 * Never discover a save by scanning a directory for matching contents. Atomic
 * writes deliberately create a same-directory `.tmp` file containing the final
 * bytes before rename, and a polling test can otherwise capture that transient
 * path. The title bar is updated only from Rust's committed document descriptor.
 */
export async function waitForCommittedDocument(
  options: CommittedDocumentOptions,
): Promise<string> {
  let observedPath: string | null = null;
  let observedDetail = "no saved document identity";

  await waitFor(
    async () => {
      observedPath = await titleBar.documentPath();
      if (!observedPath || path.dirname(observedPath) !== options.directory) {
        observedDetail = `path=${JSON.stringify(observedPath)}`;
        return false;
      }

      try {
        const actual = fs.readFileSync(observedPath, "utf8");
        observedDetail = `path=${JSON.stringify(observedPath)}, bytes=${actual.length}`;
        return actual === options.content;
      } catch (error) {
        observedDetail = `path=${JSON.stringify(observedPath)}, read=${String(error)}`;
        return false;
      }
    },
    {
      message: () =>
        `The active document never reached its committed path with the expected content. ` +
        `Last observation: ${observedDetail}.`,
      timeoutMs: options.timeoutMs ?? TIMEOUTS.disk,
    },
  );

  await waitForAppStable({
    message: "The document was committed but its application state did not settle.",
    timeoutMs: options.timeoutMs ?? TIMEOUTS.disk,
  });

  const finalPath = await titleBar.documentPath();
  if (
    finalPath === null ||
    finalPath !== observedPath ||
    path.dirname(finalPath) !== options.directory ||
    fs.readFileSync(finalPath, "utf8") !== options.content
  ) {
    throw new Error(
      `Committed document identity changed after settling: ` +
        `${JSON.stringify(observedPath)} -> ${JSON.stringify(finalPath)}.`,
    );
  }

  return finalPath;
}
