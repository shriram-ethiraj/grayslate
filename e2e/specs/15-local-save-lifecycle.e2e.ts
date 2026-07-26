import fs from "node:fs";
import path from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  clickTestId,
  externalRoot,
  openAuthorizedPath,
  openExternalText,
  pressMod,
  replaceEditorText,
  requestNewSlate,
  waitForDirtyState,
  waitForEditorReady,
  waitForFile,
  waitForSaveActionEnabled,
} from "../helpers/app.js";
import { notesRoot } from "../helpers/sandbox.js";

function regularFiles(directory: string): string[] {
  return fs.readdirSync(directory)
    .map((name) => path.join(directory, name))
    .filter((candidate) => fs.statSync(candidate).isFile())
    .sort();
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
      // The save selected from the close guard may still be in flight.
    }
    await new Promise<void>((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`Close-time local save did not update ${filePath}`);
}

describe("Act 15 — comprehensive local-file save lifecycle", () => {
  it("saves only on command and keeps the same path through repeated saves, switching, and close", async () => {
    const initial = "local lifecycle: initial";
    const firstSave = "local lifecycle: first explicit save";
    const secondSave = "local lifecycle: second explicit save";
    const switchSave = "local lifecycle: save from the switch guard";
    const closeSave = "local lifecycle: save from the close guard";
    const localPath = await openExternalText("local-save-lifecycle.txt", initial);
    const initialLocalInventory = regularFiles(externalRoot);
    const initialSlateInventory = regularFiles(notesRoot);

    expect(initialLocalInventory).toEqual([localPath]);
    expect(initialSlateInventory).toEqual([]);
    await waitForDirtyState(false);
    await waitForSaveActionEnabled(false);

    await replaceEditorText(firstSave);
    await waitForDirtyState(true);
    await waitForSaveActionEnabled(true);

    // Local files must never be changed by the slate autosave timer.
    await browser.pause(2_500);
    expect(fs.readFileSync(localPath, "utf8")).toBe(initial);
    expect(regularFiles(externalRoot)).toEqual(initialLocalInventory);
    expect(regularFiles(notesRoot)).toEqual(initialSlateInventory);

    await pressMod("s");
    await pressMod("s");
    await pressMod("s");
    await waitForFile(localPath, (content) => content === firstSave);
    await waitForDirtyState(false);
    await waitForSaveActionEnabled(false);
    expect(regularFiles(externalRoot)).toEqual(initialLocalInventory);
    expect(regularFiles(notesRoot)).toEqual(initialSlateInventory);

    await replaceEditorText(secondSave);
    await waitForDirtyState(true);
    await pressMod("s");
    await waitForFile(localPath, (content) => content === secondSave);
    await waitForDirtyState(false);
    expect(regularFiles(externalRoot)).toEqual(initialLocalInventory);

    await replaceEditorText(switchSave);
    await requestNewSlate();
    const switchDialog = await $("[data-testid='unsaved-changes-dialog']");
    await switchDialog.waitForDisplayed();
    await clickTestId("unsaved-save");
    await switchDialog.waitForDisplayed({ reverse: true });
    await waitForFile(localPath, (content) => content === switchSave);
    await waitForEditorReady({
      documentPath: "New Slate",
      documentLength: 0,
    });
    expect(regularFiles(externalRoot)).toEqual(initialLocalInventory);
    expect(regularFiles(notesRoot)).toEqual(initialSlateInventory);

    await openAuthorizedPath(localPath);
    await replaceEditorText(closeSave);
    await waitForDirtyState(true);
    await (await $("button[aria-label='Close']")).click();
    const closeDialog = await $("[data-testid='unsaved-changes-dialog']");
    await closeDialog.waitForDisplayed();
    await clickTestId("unsaved-save");

    await waitForDiskWithoutWebDriver(localPath, closeSave);
    expect(regularFiles(externalRoot)).toEqual(initialLocalInventory);
    expect(regularFiles(notesRoot)).toEqual(initialSlateInventory);
  });
});
