import fs from "node:fs";
import path from "node:path";
import { browser, expect } from "@wdio/globals";
import { scenario } from "../coverage/scenario.js";
import { forceAutosaveCycle } from "../driver/autosave.js";
import { readNativeWindowStates } from "../driver/probe.js";
import { MOD } from "../driver/keys.js";
import { waitFor, waitForAppStable } from "../driver/wait.js";
import { TIMEOUTS } from "../config/timeouts.js";
import { armOperationGate, waitForOperationGate, releaseOperationGate } from "../driver/operationGate.js";
import {
  activatePaths, dropPaths, openText, provisionText, provisionBytes,
  queueOpenDialogResults, queueSaveDialogResult, notesRoot, externalRoot, writeLargeCsv, openPath,
} from "../fixtures/factories.js";
import * as app from "../pages/app.js";
import * as editor from "../pages/editor.js";
import * as titleBar from "../pages/titleBar.js";
import * as dialogs from "../pages/dialogs.js";
import * as document from "../pages/document.js";
import * as csv from "../pages/csv.js";

async function otherWindow(original: string): Promise<string> {
  await waitFor(async () => (await browser.getWindowHandles()).some((handle) => handle !== original), {
    timeoutMs: TIMEOUTS.editor,
    message: "The requested second window did not appear.",
  });
  const other = (await browser.getWindowHandles()).find((handle) => handle !== original);
  if (!other) throw new Error("Second window disappeared before switching.");
  return other;
}

describe("Ownership-aware multi-window opening", () => {
  scenario("window.autosave-isolation", "keeps autosave identities and content isolated between windows", async () => {
    const first = await browser.getWindowHandle();
    await titleBar.fileMenu("new-window");
    const second = await otherWindow(first);
    await browser.switchToWindow(second);
    await editor.waitUntilReady({ documentPath: "New Slate", documentLength: 0 });
    await browser.switchToWindow(first);
    await editor.replaceText("first window unique slate content\n");
    await forceAutosaveCycle();
    const firstPath = await document.waitForCommittedDocument({ content: "first window unique slate content\n", directory: notesRoot });
    await browser.switchToWindow(second);
    await editor.waitUntilReady({ documentPath: "New Slate", documentLength: 0 });
    await editor.replaceText("second window independent slate content\n");
    const secondPath = await document.waitForCommittedDocument({ content: "second window independent slate content\n", directory: notesRoot });
    expect(secondPath).not.toBe(firstPath);
    expect(fs.readFileSync(firstPath, "utf8")).toBe("first window unique slate content\n");
    await titleBar.closeWindow();
    await browser.switchToWindow(first);
    await editor.waitUntilReady({ documentPath: firstPath });
    await editor.waitForExactText("first window unique slate content\n");
  });

  scenario("file.slate.new-identity", "starts an independent autosave destination after New Slate", async () => {
    await app.newSlate();
    const originalContent = "original saved slate destination\n";
    await editor.replaceText(originalContent);
    await forceAutosaveCycle();
    const original = await document.waitForCommittedDocument({ content: originalContent, directory: notesRoot });
    await app.newSlate();
    const nextContent = "new slate independent destination\n";
    await editor.replaceText(nextContent);
    await forceAutosaveCycle();
    const next = await document.waitForCommittedDocument({ content: nextContent, directory: notesRoot });
    expect(next).not.toBe(original);
    expect(fs.readFileSync(original, "utf8")).toBe(originalContent);
  });

  scenario("file.batch.partial-read-failure", "opens preceding files even when the final file is binary", async () => {
    const first = await browser.getWindowHandle();
    const source = await openText("partial-source.txt", "source remains\n");
    const valid = provisionText("partial-valid.txt", "valid preceding file\n");
    const binary = provisionBytes("partial-binary.bin", Buffer.from([0xff, 0xfe, 0, 0, 65, 0, 0, 0]));
    await queueOpenDialogResults([valid, binary]);
    await titleBar.fileMenu("open-file");
    await editor.waitUntilReady({ documentPath: source });
    const second = await otherWindow(first);
    await browser.switchToWindow(second);
    await editor.waitUntilReady({ documentPath: valid });
    await editor.waitForExactText("valid preceding file\n");
    await waitFor(async () => (await readNativeWindowStates()).some((state) => state.is_focused && state.title.includes("partial-valid.txt")), {
      timeoutMs: TIMEOUTS.ui, message: "A failed final file prevented the last successful file receiving focus.",
    });
    await titleBar.closeWindow();
    await browser.switchToWindow(first);
    await editor.waitForExactText("source remains\n");
  });

  scenario("file.batch.duplicate-paths", "deduplicates repeated paths before applying the request allowance", async () => {
    const target = provisionText("duplicate-batch.txt", "one unique file\n");
    await dropPaths(Array.from({ length: 60 }, () => target));
    await waitForAppStable();
    await editor.waitUntilReady({ documentPath: target });
    expect(await browser.getWindowHandles()).toHaveLength(1);
    await editor.waitForExactText("one unique file\n");
  });

  scenario("file.batch.declined-encoding", "opens preceding files while preserving the source after an encoding prompt is declined", async () => {
    const first = await browser.getWindowHandle();
    const source = await openText("encoding-batch-source.txt", "source survives encoding cancellation\n");
    const valid = provisionText("encoding-batch-valid.txt", "preceding file survives\n");
    const legacy = provisionBytes("encoding-batch-legacy.txt", Buffer.from([0x63, 0x61, 0x66, 0xe9]));
    await queueOpenDialogResults([valid, legacy]);
    await titleBar.fileMenu("open-file");
    await dialogs.encodingConfirmation.waitForOpen();
    await dialogs.encodingConfirmation.cancel();
    await editor.waitUntilReady({ documentPath: source });
    await editor.waitForExactText("source survives encoding cancellation\n");
    const second = await otherWindow(first);
    await browser.switchToWindow(second);
    await editor.waitUntilReady({ documentPath: valid });
    await editor.waitForExactText("preceding file survives\n");
    await titleBar.closeWindow();
    await browser.switchToWindow(first);
    expect(fs.readFileSync(legacy)).toEqual(Buffer.from([0x63, 0x61, 0x66, 0xe9]));
  });

  scenario("file.os-open.preserves-occupied", "preserves dirty external files and reuses existing owners on OS activation", async () => {
    const first = await browser.getWindowHandle();
    const source = await openText("os-source.txt", "saved source\n");
    const target = provisionText("os-target.txt", "OS opened content\n");
    await editor.replaceText("unsaved source\n");
    await activatePaths([target]);
    await waitForAppStable();
    expect(await dialogs.unsavedChanges.isOpen()).toBe(false);
    expect(await titleBar.documentPath()).toBe(source);
    expect(await editor.text()).toBe("unsaved source\n");
    const second = await otherWindow(first);
    await browser.switchToWindow(second);
    await editor.waitUntilReady({ documentPath: target });
    await browser.switchToWindow(first);
    await activatePaths([target]);
    await waitForAppStable();
    expect(await browser.getWindowHandles()).toHaveLength(2);
    expect(await editor.text()).toBe("unsaved source\n");
    await titleBar.fileMenu("save");
    await titleBar.waitForDirty(false);
    await browser.switchToWindow(second);
    await titleBar.closeWindow();
    await browser.switchToWindow(first);
  });

  scenario("file.os-open.reuses-blank", "reuses an empty new slate for an OS activation", async () => {
    await waitFor(async () => (await browser.getWindowHandles()).length === 1, {
      timeoutMs: TIMEOUTS.editor,
      message: "The preceding window did not finish closing.",
    });
    await app.newSlate();
    await editor.focus();
    const target = provisionText("os-blank.txt", "reuse the empty window\n");
    await activatePaths([target]);
    await waitForAppStable();
    await editor.waitUntilReady({ documentPath: target });
    expect(await browser.getWindowHandles()).toHaveLength(1);
  });

  scenario("file.csv.large-unsaved-guard", "protects CSV edits above the live-mirroring threshold", async () => {
    const source = path.join(externalRoot, "large-dirty.csv");
    writeLargeCsv(source, 100_001);
    await openPath(source);
    await csv.enter();
    await titleBar.waitForDirty(false);
    const cleanTarget = provisionText("after-clean-csv.txt", "clean CSV needs no guard\n");
    await dropPaths([cleanTarget]);
    await editor.waitUntilReady({ documentPath: cleanTarget });
    expect(await dialogs.unsavedChanges.isOpen()).toBe(false);
    await openPath(source);
    await csv.enter();
    await csv.editCell(0, 1, "unsaved table value");
    await titleBar.waitForDirty(true);
    const target = provisionText("after-large-csv.txt", "replacement\n");
    await dropPaths([target]);
    await dialogs.unsavedChanges.waitForOpen();
    await dialogs.unsavedChanges.cancel();
    await dialogs.unsavedChanges.waitForClosed();
    expect(await titleBar.documentPath()).toBe(source);
    await csv.waitForCellText(0, 1, "unsaved table value");
    expect(fs.readFileSync(source, "utf8")).toContain("1,row-1,2");
    await titleBar.fileMenu("save");
    await titleBar.waitForDirty(false);
    await csv.editCell(0, 1, "second table edit");
    await dropPaths([target]);
    await dialogs.unsavedChanges.waitForOpen();
    await dialogs.unsavedChanges.save();
    await editor.waitUntilReady({ documentPath: target });
    expect(fs.readFileSync(source, "utf8")).toContain("1,second table edit,2");
  });

  scenario("file.save-as.current-owner", "allows Save As to write the current owned path", async () => {
    const target = await openText("save-as-same-path.txt", "before\n");
    await editor.replaceText("saved to the same owned path\n");
    await queueSaveDialogResult(target);
    await titleBar.fileMenu("save-as");
    await titleBar.waitForDirty(false);
    expect(fs.readFileSync(target, "utf8")).toBe("saved to the same owned path\n");
    expect(await browser.getWindowHandles()).toHaveLength(1);
    await editor.waitUntilReady({ documentPath: target });
  });

  scenario("file.open.pending-owner", "reuses a window whose document is still loading", async () => {
    const first = await browser.getWindowHandle();
    const source = await openText("pending-owner-source.txt", "original window\n");
    const target = provisionText("pending-owner-target.txt", "one loading owner\n");
    await armOperationGate("file-read");
    try {
      await queueOpenDialogResults([target]);
      await titleBar.fileMenu("open-file-new-window");
      await waitForOperationGate("file-read");
      // The native request races the second window's held read. It must find
      // the reservation rather than create a third window for the same path.
      await activatePaths([target]);
    } finally {
      await releaseOperationGate("file-read");
    }
    await waitForAppStable();
    expect(await browser.getWindowHandles()).toHaveLength(2);
    await editor.waitUntilReady({ documentPath: source });
    const second = await otherWindow(first);
    await browser.switchToWindow(second);
    await editor.waitUntilReady({ documentPath: target });
    await editor.waitForExactText("one loading owner\n");
    await titleBar.closeWindow();
    await browser.switchToWindow(first);
    await editor.waitForExactText("original window\n");
  });

  scenario("file.large-text.immediate-guard", "protects the live large-document edit before debounced text synchronization", async () => {
    // Cross the 5,000,000-character serialization threshold without typing
    // megabytes through WebDriver. Only the final edit is entered via keys.
    const original = `${"large local guard row ".repeat(3)}\n`.repeat(100_001);
    const source = await openText("large-local-guard.txt", original);
    const target = provisionText("after-large-local.txt", "replacement after large text\n");
    await queueOpenDialogResults([target]);
    await editor.focus();
    // One native key sequence requests Open immediately after the edit; no
    // dirty-state poll is allowed to let the serialization debounce finish.
    await browser.action("key")
      .down("x").up("x")
      .down(MOD).down("o").up("o").up(MOD)
      .perform();
    await dialogs.unsavedChanges.waitForOpen();
    await dialogs.unsavedChanges.cancel();
    await dialogs.unsavedChanges.waitForClosed();
    await editor.waitUntilReady({ documentPath: source, documentLength: original.length + 1 });
    expect(fs.readFileSync(source, "utf8")).toBe(original);
    await queueOpenDialogResults([target]);
    await titleBar.fileMenu("open-file");
    await dialogs.unsavedChanges.waitForOpen();
    await dialogs.unsavedChanges.save();
    await editor.waitUntilReady({ documentPath: target });
    const saved = fs.readFileSync(source, "utf8");
    expect(saved.length).toBe(original.length + 1);
    expect(saved.replace("x", "")).toBe(original);
  });

  scenario("window.primary-close-survivor", "keeps autosave and OS opens working after the primary window closes", async () => {
    const primary = await browser.getWindowHandle();
    await app.newSlate();
    await titleBar.fileMenu("new-window");
    const survivor = await otherWindow(primary);
    await browser.switchToWindow(survivor);
    await editor.waitUntilReady({ documentPath: "New Slate", documentLength: 0 });
    await editor.replaceText("surviving slate before primary close\n");
    await forceAutosaveCycle();
    const saved = await document.waitForCommittedDocument({ content: "surviving slate before primary close\n", directory: notesRoot });
    await browser.switchToWindow(primary);
    await titleBar.closeWindow();
    await browser.switchToWindow(survivor);
    await waitFor(async () => (await browser.getWindowHandles()).length === 1, {
      timeoutMs: TIMEOUTS.editor,
      message: "The original primary window did not finish closing.",
    });
    await editor.replaceText("surviving slate after primary close\n");
    await forceAutosaveCycle();
    expect(await document.waitForCommittedDocument({ content: "surviving slate after primary close\n", directory: notesRoot })).toBe(saved);
    const target = provisionText("after-primary-close.txt", "OS open still works\n");
    await activatePaths([target]);
    await waitForAppStable();
    await editor.waitUntilReady({ documentPath: saved });
    const opened = await otherWindow(survivor);
    await browser.switchToWindow(opened);
    await editor.waitUntilReady({ documentPath: target });
    await editor.waitForExactText("OS open still works\n");
    await titleBar.closeWindow();
    await browser.switchToWindow(survivor);
  });
});
