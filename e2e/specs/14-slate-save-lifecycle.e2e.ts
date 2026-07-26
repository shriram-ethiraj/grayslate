import fs from "node:fs";
import path from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  newSlate,
  openAuthorizedPath,
  pressMod,
  replaceEditorText,
  waitForEditorReady,
  waitForFile,
} from "../helpers/app.js";
import { notesRoot } from "../helpers/sandbox.js";

function regularFiles(directory: string): string[] {
  return fs.readdirSync(directory)
    .map((name) => path.join(directory, name))
    .filter((candidate) => fs.statSync(candidate).isFile())
    .sort();
}

async function waitForCreatedSlate(content: string): Promise<string> {
  let createdPath = "";
  await browser.waitUntil(() => {
    const matches = regularFiles(notesRoot).filter(
      (candidate) => fs.readFileSync(candidate, "utf8") === content,
    );
    if (matches.length !== 1) return false;
    createdPath = matches[0] ?? "";
    return createdPath !== "";
  }, {
    timeout: 20_000,
    interval: 200,
    timeoutMsg: "The first slate autosave did not create exactly one matching file.",
  });
  return createdPath;
}

async function waitForDiskWithoutWebDriver(
  filePath: string,
  expected: string,
  timeoutMs = 15_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      if (fs.readFileSync(filePath, "utf8") === expected) return;
    } catch {
      // The close-time write may not have reached disk yet.
    }
    await new Promise<void>((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`Close-time slate save did not update ${filePath}`);
}

describe("Act 14 — comprehensive slate save lifecycle", () => {
  it("keeps one file identity across autosave, manual save, switching, and close", async () => {
    const first = "slate lifecycle: first autosave";
    const second = "slate lifecycle: second autosave updates the same file";
    const third = "slate lifecycle: repeated manual saves update the same file";
    const switched = "slate lifecycle: switching flushes the same file";
    const closed = "slate lifecycle: closing flushes the same file";

    await replaceEditorText(first);
    const slatePath = await waitForCreatedSlate(first);
    const initialInventory = regularFiles(notesRoot);
    expect(initialInventory).toEqual([slatePath]);
    await waitForEditorReady({
      documentPath: slatePath,
      documentLength: first.length,
    });

    // This is the original regression: after the first autosave establishes
    // the file identity, the next edit must overwrite that path, not create a
    // second content-named slate.
    await replaceEditorText(second);
    await waitForFile(slatePath, (content) => content === second, 20_000);
    expect(regularFiles(notesRoot)).toEqual(initialInventory);
    await waitForEditorReady({
      documentPath: slatePath,
      documentLength: second.length,
    });

    await replaceEditorText(third);
    await pressMod("s");
    await pressMod("s");
    await pressMod("s");
    await waitForFile(slatePath, (content) => content === third);
    await browser.pause(2_500);
    expect(regularFiles(notesRoot)).toEqual(initialInventory);

    await replaceEditorText(switched);
    await newSlate();
    await waitForFile(slatePath, (content) => content === switched);
    expect(regularFiles(notesRoot)).toEqual(initialInventory);

    await openAuthorizedPath(slatePath);
    await replaceEditorText(closed);
    await (await $("button[aria-label='Close']")).click();

    // The app window is destroyed after the backend flush, so finish with a
    // Node-side disk poll that does not depend on a live WebDriver webview.
    await waitForDiskWithoutWebDriver(slatePath, closed);
    expect(regularFiles(notesRoot)).toEqual(initialInventory);
  });
});
