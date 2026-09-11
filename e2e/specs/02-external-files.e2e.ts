import fs from "node:fs";
import path from "node:path";
import { browser, expect } from "@wdio/globals";
import { directoryInventory, expectNoNewFiles } from "../assertions/matchers.js";
import { scenario } from "../coverage/scenario.js";
import {
  armOperationGate,
  releaseOperationGate,
  waitForOperationGate,
} from "../driver/operationGate.js";
import { waitForAppStable, waitForFile } from "../driver/wait.js";
import {
  dropPaths,
  externalRoot,
  notesRoot,
  openFixture,
  openText,
  provisionText,
  provisionSparseFile,
  queueOpenDialogCancel,
  queueOpenDialogResult,
  queueOpenDialogResults,
  queueSaveDialogCancel,
  queueSaveDialogResult,
  requestOpenPath,
} from "../fixtures/factories.js";
import * as app from "../pages/app.js";
import * as dialogs from "../pages/dialogs.js";
import * as editor from "../pages/editor.js";
import * as sidebar from "../pages/sidebar.js";
import * as statusBar from "../pages/statusBar.js";
import * as titleBar from "../pages/titleBar.js";
import * as transformations from "../pages/transformations.js";
import { clickTestId, waitForTestId } from "../pages/common.js";

/**
 * External (local) files.
 *
 * Open and Save As run through the real menu items. The native dialogs cannot
 * be automated, so the spec pre-selects the answer the user would have given
 * and the production command consumes it. That means the menu wiring, the
 * authorization, the descriptor handling, and the frontend's open/save flow are
 * all genuinely exercised — the previous version wrote the file over IPC and
 * proved none of it.
 */
describe("External files", () => {
  scenario(
    "file.open.active-feedback",
    "reports the active file across sidebar, picker, and external open attempts",
    async () => {
      const target = await openText("already-open.txt", "saved active text\n");
      const edited = "active unsaved text";
      const expectedToast = '"already-open.txt" is already open in this window.';

      await editor.replaceText(edited);
      await titleBar.waitForDirty(true);

      await sidebar.ensureOpen();
      await sidebar.setFilterTab("unified");
      await sidebar.waitForCard(target);
      await sidebar.openCard(target);
      await transformations.waitForToast(expectedToast);
      await clickTestId("toast");
      await waitForTestId("toast", { reverse: true });

      await queueOpenDialogResult(target);
      await titleBar.fileMenu("open-file");
      await transformations.waitForToast(expectedToast);
      await clickTestId("toast");
      await waitForTestId("toast", { reverse: true });

      await queueOpenDialogResult(target);
      await titleBar.fileMenu("open-file-new-window");
      await transformations.waitForToast(expectedToast);
      await clickTestId("toast");
      await waitForTestId("toast", { reverse: true });

      // The external route must surface the same feedback without reloading.
      await dropPaths([target]);
      await transformations.waitForToast(expectedToast);
      await waitForAppStable({
        message: "Reopening the active document did not settle.",
      });

      expect(await dialogs.unsavedChanges.isOpen()).toBe(false);
      await editor.waitUntilReady({ documentPath: target });
      expect(await editor.text()).toBe(edited);
      expect(await titleBar.isDirty()).toBe(true);

      // Leave the shared app process clean for the remaining scenarios in
      // this spec. Otherwise the next File > Open correctly stops at the
      // unsaved-changes guard instead of exercising its intended open flow.
      await titleBar.fileMenu("save");
      await titleBar.waitForDirty(false);
    },
  );

  scenario(
    "file.menu-open.multiple",
    "opens every File menu selection and keeps the final path in the current window",
    async () => {
      const mainHandle = await browser.getWindowHandle();
      const first = provisionText("menu-open-first.json", "{\"menu\":1}\n");
      const last = provisionText("menu-open-last.py", "print('menu final')\n");

      await queueOpenDialogResults([first, last]);
      await titleBar.fileMenu("open-file");

      await editor.waitUntilReady({ documentPath: last });
      const handles = await browser.getWindowHandles();
      expect(handles).toHaveLength(2);
      const secondary = handles.find((handle) => handle !== mainHandle);
      if (!secondary) throw new Error("The preceding File menu selection did not open a window.");
      await browser.switchToWindow(secondary);
      await editor.waitUntilReady({ documentPath: first });
      await browser.closeWindow();
      await browser.switchToWindow(mainHandle);
    },
  );

  scenario(
    "file.external.open",
    "opens an external file through the File menu, under All and Local",
    async () => {
      const target = provisionText("menu-open.py", "import sys\nprint(sys.argv)\n");

      await queueOpenDialogResult(target);
      await titleBar.fileMenu("open-file");

      await editor.waitUntilReady({ documentPath: target });
      await statusBar.waitForDetectedLanguage("python");

      await sidebar.ensureOpen();
      await sidebar.setFilterTab("unified");
      await sidebar.waitForCard(target);
      await sidebar.setFilterTab("local");
      await sidebar.waitForCard(target);
      // An external file is not a slate and must never be listed as one.
      await sidebar.setFilterTab("slates");
      await sidebar.waitForCard(target, false);
      await sidebar.setFilterTab("unified");
    },
  );

  scenario(
    "file.drop.multiple",
    "tracks and opens every dropped file while keeping the final path in the target window",
    async () => {
      const mainHandle = await browser.getWindowHandle();
      const first = provisionText("drop-first.json", "{\"order\":1}\n");
      const last = provisionText("drop-last.py", "print('opened last')\n");
      const missing = path.join(externalRoot, "drop-missing.txt");

      await dropPaths([first, missing, last]);

      await editor.waitUntilReady({ documentPath: last });
      await statusBar.waitForDetectedLanguage("python");
      await transformations.waitForToastContaining("1 file could not be opened");
      await sidebar.ensureOpen();
      await sidebar.setFilterTab("local");
      await sidebar.waitForCard(first);
      await sidebar.waitForCard(last);
      await sidebar.setFilterTab("unified");

      const handles = await browser.getWindowHandles();
      expect(handles).toHaveLength(2);
      const secondary = handles.find((handle) => handle !== mainHandle);
      if (!secondary) throw new Error("The preceding dropped file did not open a window.");
      await browser.switchToWindow(secondary);
      await editor.waitUntilReady({ documentPath: first });
      await browser.closeWindow();
      await browser.switchToWindow(mainHandle);
      await editor.waitUntilReady({ documentPath: last });
    },
  );

  scenario(
    "file.drop.invalid-batch",
    "warns and keeps the editor usable when every dropped path is invalid",
    async () => {
      const source = await openText("drop-invalid-source.txt", "source remains\n");
      const oversized = provisionSparseFile(
        "drop-over-200mb.txt",
        200 * 1024 * 1024 + 1,
      );

      await dropPaths([externalRoot, oversized]);

      await transformations.waitForToastContaining("2 files could not be opened");
      await editor.waitUntilReady({ documentPath: source });
      await editor.waitForExactText("source remains\n");
      await editor.replaceText("still usable after invalid drop");
      await editor.waitForExactText("still usable after invalid drop");
    },
  );

  scenario(
    "file.drop.unsaved-guard",
    "preserves edits on Cancel or New Window and opens the drop after Discard",
    async () => {
      const mainHandle = await browser.getWindowHandle();
      const source = await openText("drop-guard-source.txt", "saved source\n");
      const target = provisionText("drop-guard-target.txt", "dropped target\n");
      const preceding = provisionText("drop-guard-preceding.txt", "preceding target\n");
      const edited = "unsaved source edit";

      await editor.replaceText(edited);
      await dropPaths([target]);
      await dialogs.unsavedChanges.waitForOpen();
      await dialogs.unsavedChanges.cancel();
      await dialogs.unsavedChanges.waitForClosed();

      await editor.waitUntilReady({ documentPath: source });
      expect(await editor.text()).toBe(edited);
      expect(await titleBar.isDirty()).toBe(true);

      await dropPaths([target]);
      await dialogs.unsavedChanges.waitForOpen();
      await dialogs.unsavedChanges.newWindow();
      await dialogs.unsavedChanges.waitForClosed();

      const handles = await browser.getWindowHandles();
      expect(handles).toHaveLength(2);
      const secondary = handles.find((handle) => handle !== mainHandle);
      if (!secondary) throw new Error("Open in New Window did not create a window.");
      await browser.switchToWindow(secondary);
      await editor.waitUntilReady({ documentPath: target });
      await browser.switchToWindow(mainHandle);
      await editor.waitUntilReady({ documentPath: source });
      expect(await editor.text()).toBe(edited);
      expect(await titleBar.isDirty()).toBe(true);

      // The final target already belongs to the secondary window, so this
      // navigation cannot replace the dirty receiving document and must not
      // show a destructive-choice dialog.
      await dropPaths([preceding, target]);
      await waitForAppStable();
      expect(await dialogs.unsavedChanges.isOpen()).toBe(false);
      expect(await browser.getWindowHandles()).toHaveLength(3);
      await editor.waitUntilReady({ documentPath: source });
      expect(await editor.text()).toBe(edited);
      expect(await titleBar.isDirty()).toBe(true);

      for (const handle of await browser.getWindowHandles()) {
        if (handle === mainHandle) continue;
        await browser.switchToWindow(handle);
        await browser.closeWindow();
      }
      await browser.switchToWindow(mainHandle);

      await dropPaths([target]);
      await dialogs.unsavedChanges.waitForOpen();
      await dialogs.unsavedChanges.discard();
      await dialogs.unsavedChanges.waitForClosed();
      await editor.waitUntilReady({ documentPath: target });
      expect(fs.readFileSync(source, "utf8")).toBe("saved source\n");
    },
  );

  scenario(
    "file.drop.dialog-queue",
    "keeps the active dialog open and processes the drop after it closes",
    async () => {
      const source = await openText("drop-dialog-source.txt", "source stays visible\n");
      const target = provisionText("drop-dialog-target.txt", "opens after dialog\n");

      await titleBar.helpMenu("about");
      await dialogs.about.waitForOpen();
      await dropPaths([target]);

      expect(await dialogs.about.text()).toContain("Grayslate");
      await editor.waitUntilReady({ documentPath: source });
      await editor.waitForExactText("source stays visible\n");

      await dialogs.about.close();
      await editor.waitUntilReady({ documentPath: target });
      await editor.waitForExactText("opens after dialog\n");
    },
  );

  scenario(
    "file.drop.all-owned-dirty",
    "bypasses the dirty guard when every dropped file already has a window",
    async () => {
      const mainHandle = await browser.getWindowHandle();
      const source = await openText("drop-owned-source.txt", "saved owner source\n");
      const first = provisionText("drop-owned-first.txt", "first owner\n");
      const last = provisionText("drop-owned-last.txt", "last owner\n");
      const edited = "dirty owner source";

      await editor.replaceText(edited);
      await dropPaths([first, last]);
      await dialogs.unsavedChanges.waitForOpen();
      await dialogs.unsavedChanges.newWindow();
      await dialogs.unsavedChanges.waitForClosed();
      expect(await browser.getWindowHandles()).toHaveLength(3);

      await browser.switchToWindow(mainHandle);
      await dropPaths([first, last]);
      await waitForAppStable();
      expect(await dialogs.unsavedChanges.isOpen()).toBe(false);
      await editor.waitUntilReady({ documentPath: source });
      expect(await editor.text()).toBe(edited);
      expect(await titleBar.isDirty()).toBe(true);

      for (const handle of await browser.getWindowHandles()) {
        if (handle === mainHandle) continue;
        await browser.switchToWindow(handle);
        await browser.closeWindow();
      }
      await browser.switchToWindow(mainHandle);
      await titleBar.fileMenu("save");
      await titleBar.waitForDirty(false);
    },
  );

  scenario(
    "file.drop.active-noop",
    "does not reload or prompt when the active file is dropped again",
    async () => {
      const target = await openText("drop-active.txt", "saved active text\n");
      const edited = "active unsaved text";

      await editor.replaceText(edited);
      await dropPaths([target]);
      await waitForAppStable({
        message: "Re-dropping the active document did not settle.",
      });

      expect(await dialogs.unsavedChanges.isOpen()).toBe(false);
      await editor.waitUntilReady({ documentPath: target });
      expect(await editor.text()).toBe(edited);
      expect(await titleBar.isDirty()).toBe(true);
    },
  );

  scenario(
    "file.dirty.local-only",
    "marks a local file dirty on edit, clears it on save, and never dirties a slate",
    async () => {
      const target = await openFixture("sample.py", "dirty-indicator.py");
      await titleBar.waitForDirty(false);

      await editor.replaceText("# edited\n");
      await titleBar.waitForDirty(true);
      await titleBar.waitForSaveEnabled(true);

      await editor.save();
      await waitForFile(target, (content) => content === "# edited\n", {
        message: "The local save never reached disk.",
      });
      await titleBar.waitForDirty(false);

      // Slates are autosaved, so the indicator must never appear for one.
      await app.newSlate();
      await editor.replaceText("slate content, never dirty\n");
      expect(await titleBar.isDirty()).toBe(false);
    },
  );

  scenario(
    "file.save-as.local",
    "saves a local file to a new path through the Save As menu item",
    async () => {
      const source = await openText("save-as-source.txt", "save-as original\n");
      const destination = path.join(externalRoot, "save-as-destination.txt");
      const before = directoryInventory(externalRoot);

      await editor.replaceText("save-as updated\n");
      await queueSaveDialogResult(destination);
      await titleBar.fileMenu("save-as");

      await waitForFile(destination, (content) => content === "save-as updated\n", {
        message: "Save As did not write the chosen path.",
      });
      // The editor follows the new path.
      await editor.waitUntilReady({ documentPath: destination });
      await titleBar.waitForDirty(false);

      expectNoNewFiles(externalRoot, before, ["save-as-destination.txt"]);
      expect(fs.readFileSync(source, "utf8")).toBe("save-as original\n");
    },
  );

  scenario(
    "file.save-as.leaves-original",
    "leaves the original untouched when Save As targets a different file",
    async () => {
      const source = await openText("save-as-keep.txt", "keep me\n");
      const destination = path.join(externalRoot, "save-as-copy.txt");

      await editor.replaceText("changed after copy\n");
      await queueSaveDialogResult(destination);
      await titleBar.fileMenu("save-as");
      await waitForFile(destination, (content) => content === "changed after copy\n", {
        message: "Save As did not write the copy.",
      });

      // The whole point of Save As: the file you started from is not modified.
      expect(fs.readFileSync(source, "utf8")).toBe("keep me\n");
    },
  );

  scenario(
    "file.save-as.untitled",
    "saves an untitled slate to a chosen path through Save As",
    async () => {
      await app.newSlate();
      await editor.replaceText("untitled saved through save as\n");

      const destination = path.join(externalRoot, "untitled-save-as.txt");
      await queueSaveDialogResult(destination);
      await titleBar.fileMenu("save-as");

      await waitForFile(destination, (c) => c === "untitled saved through save as\n", {
        message: "Save As from an untitled slate did not write the chosen path.",
      });
      await editor.waitUntilReady({ documentPath: destination });
    },
  );

  scenario(
    "file.open.dialog-cancel",
    "changes nothing when the user cancels the Open dialog",
    async () => {
      const current = await openText("open-cancel-current.txt", "still open\n");
      const beforeNotes = directoryInventory(notesRoot);
      const beforeExternal = directoryInventory(externalRoot);

      // A queued cancellation, not an empty queue: an empty queue means "no
      // test opinion" and lets the real native dialog open, which blocks.
      await queueOpenDialogCancel();
      await titleBar.fileMenu("open-file");

      // The document under the cursor must be exactly what it was. The file ends
      // in a newline, so the document does too — `waitForExactText` compares the
      // whole document, trailing newline included.
      await editor.waitUntilReady({ documentPath: current });
      await editor.waitForExactText("still open\n");
      await titleBar.waitForDirty(false);

      expectNoNewFiles(notesRoot, beforeNotes, []);
      expectNoNewFiles(externalRoot, beforeExternal, []);
    },
  );

  scenario(
    "file.save-as.dialog-cancel",
    "writes nothing when the user cancels the Save As dialog",
    async () => {
      const source = await openText("save-as-cancel.txt", "original bytes\n");
      const before = directoryInventory(externalRoot);

      await editor.replaceText("edited but not saved\n");
      await titleBar.waitForDirty(true);

      await queueSaveDialogCancel();
      await titleBar.fileMenu("save-as");

      // Cancelling must not write, must not move the document, and must not
      // silently discard the edit — the user still has unsaved work.
      expectNoNewFiles(externalRoot, before, []);
      expect(fs.readFileSync(source, "utf8")).toBe("original bytes\n");
      await editor.waitUntilReady({ documentPath: source });
      await titleBar.waitForDirty(true);

      // The document is still usable: a real save afterwards lands normally.
      await editor.save();
      await waitForFile(source, (content) => content === "edited but not saved\n", {
        message: "Saving after a cancelled Save As did not reach disk.",
      });
    },
  );

  scenario(
    "file.guard.cancel-and-discard",
    "guards unsaved local changes across Cancel, New Window, and Discard",
    async () => {
      const mainHandle = await browser.getWindowHandle();
      const target = await openText("guard-cancel.txt", "guard base\n");
      const edited = "guard: unsaved edit";

      await editor.replaceText(edited);
      await titleBar.waitForDirty(true);

      // Cancel keeps both the document and the edit.
      await app.requestNewSlate();
      await dialogs.unsavedChanges.waitForOpen();
      await dialogs.unsavedChanges.cancel();
      await dialogs.unsavedChanges.waitForClosed();
      expect(await editor.text()).toBe(edited);
      expect(await titleBar.isDirty()).toBe(true);

      // New Window creates the slate elsewhere without touching this edit.
      await app.requestNewSlate();
      await dialogs.unsavedChanges.waitForOpen();
      await dialogs.unsavedChanges.newWindow();
      await dialogs.unsavedChanges.waitForClosed();
      const handles = await browser.getWindowHandles();
      expect(handles).toHaveLength(2);
      const secondary = handles.find((handle) => handle !== mainHandle);
      if (!secondary) throw new Error("Create in New Window did not create a window.");
      await browser.switchToWindow(secondary);
      await editor.waitUntilReady({ documentPath: "New Slate", documentLength: 0 });
      await browser.closeWindow();
      await browser.switchToWindow(mainHandle);
      expect(await editor.text()).toBe(edited);
      expect(await titleBar.isDirty()).toBe(true);

      // Discard drops the edit and leaves the file on disk untouched.
      await app.requestNewSlate();
      await dialogs.unsavedChanges.waitForOpen();
      await dialogs.unsavedChanges.discard();
      await dialogs.unsavedChanges.waitForClosed();
      await editor.waitUntilReady({ documentPath: "New Slate", documentLength: 0 });
      expect(fs.readFileSync(target, "utf8")).toBe("guard base\n");
    },
  );

  scenario(
    "file.read.cancel",
    "cancels an in-flight read and leaves the editor usable",
    async () => {
      await openText("read-cancel-base.txt", "base document\n");
      const target = provisionText("read-cancel-target.txt", "target document\n");

      await armOperationGate("file-read");
      await requestOpenPath(target);
      await waitForOperationGate("file-read");

      try {
        // Starting a new document invalidates the open request and invokes the
        // real cancel_file_read path while the Rust worker is held. Only issue
        // the action here: waiting for the blank editor before releasing the
        // worker creates a circular wait in WebKit's raw-response transport.
        await app.requestNewSlate();
      } finally {
        await releaseOperationGate("file-read");
      }

      await editor.waitUntilReady({ documentPath: "New Slate", documentLength: 0 });
      await editor.replaceText("still usable after cancellation");
      await editor.waitForExactText("still usable after cancellation");
    },
  );

  scenario(
    "file.size-limit",
    "rejects a sparse file above 200 MB before reading its content",
    async () => {
      await openText("size-limit-base.txt", "base remains open\n");
      const oversized = provisionSparseFile(
        "over-200mb.txt",
        200 * 1024 * 1024 + 1,
      );

      await requestOpenPath(oversized);
      await transformations.waitForToastContaining("maximum allowed size is 200 MB");
      await editor.waitForExactText("base remains open\n");
      await editor.replaceText("usable after size rejection");
      await editor.waitForExactText("usable after size rejection");
    },
  );

});
