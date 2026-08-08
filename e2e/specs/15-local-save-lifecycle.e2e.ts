import fs from "node:fs";
import path from "node:path";
import { expect } from "@wdio/globals";
import { TIMEOUTS } from "../config/timeouts.js";
import {
  directoryInventory,
  expectNoNewFiles,
} from "../assertions/matchers.js";
import { scenario } from "../coverage/scenario.js";
import { forceAutosaveCycle } from "../driver/autosave.js";
import { waitForFile, waitForFileWithoutWebDriver } from "../driver/wait.js";
import { externalRoot, notesRoot, openPath, openText } from "../fixtures/factories.js";
import * as app from "../pages/app.js";
import * as dialogs from "../pages/dialogs.js";
import * as editor from "../pages/editor.js";
import * as titleBar from "../pages/titleBar.js";

/**
 * The local-file save contract.
 *
 * The load-bearing property is negative: a local file is written *only* when
 * the user says so. Autosave owns slates and must never touch an external file,
 * and no save path may quietly fork a second file.
 *
 * Each scenario seeds its own document and measures file-count changes against
 * a baseline it captures itself, so any one can run alone and none depends on
 * a sibling having run first.
 */
describe("Local file save lifecycle", () => {
  const INITIAL = "local lifecycle: initial";

  interface Seeded {
    localPath: string;
    fileName: string;
    externalBefore: string[];
    slatesBefore: string[];
  }

  async function seedLocalFile(fileName: string): Promise<Seeded> {
    const localPath = await openText(fileName, INITIAL);
    await titleBar.waitForDirty(false);
    await titleBar.waitForSaveEnabled(false);
    return {
      localPath,
      fileName,
      externalBefore: directoryInventory(externalRoot),
      slatesBefore: directoryInventory(notesRoot),
    };
  }

  scenario(
    "file.autosave.never-touches-local",
    "leaves an edited local file untouched until the user saves",
    async () => {
      const seeded = await seedLocalFile("autosave-never-touches.txt");

      await editor.replaceText("local lifecycle: edited but not saved");
      await titleBar.waitForDirty(true);
      await titleBar.waitForSaveEnabled(true);

      const forcedCycle = await forceAutosaveCycle();
      expect(forcedCycle.source).toBe("local");
      // Rust intentionally tracks autosave generations only for slates. The
      // user-visible dirty state below is authoritative for a local document.
      expect(forcedCycle.backendDirty).toBe(false);
      expect(forcedCycle.scheduledActions).toBe(0);
      expect(fs.readFileSync(seeded.localPath, "utf8")).toBe(INITIAL);
      expect(directoryInventory(notesRoot)).toHaveLength(
        seeded.slatesBefore.length,
      );

      // Still dirty: nothing silently saved it behind the user's back.
      expect(await titleBar.isDirty()).toBe(true);
    },
  );

  scenario(
    "file.save.coalesces",
    "collapses repeated Save presses into a single write to one path",
    async () => {
      const seeded = await seedLocalFile("save-coalesces.txt");
      const expected = "local lifecycle: first explicit save";

      await editor.replaceText(expected);
      await titleBar.waitForDirty(true);

      await editor.save();
      await editor.save();
      await editor.save();

      await waitForFile(seeded.localPath, (content) => content === expected, {
        message: `Save never wrote the expected content to ${seeded.localPath}.`,
      });
      await titleBar.waitForDirty(false);
      await titleBar.waitForSaveEnabled(false);

      // Three saves, one file: no fork into a second document anywhere.
      expectNoNewFiles(externalRoot, seeded.externalBefore);
      expectNoNewFiles(notesRoot, seeded.slatesBefore);
    },
  );

  scenario(
    "file.external.save",
    "writes each subsequent explicit save to the same path",
    async () => {
      const seeded = await seedLocalFile("repeated-save.txt");

      for (const content of ["local lifecycle: second", "local lifecycle: third"]) {
        await editor.replaceText(content);
        await titleBar.waitForDirty(true);
        await editor.save();
        await waitForFile(seeded.localPath, (value) => value === content, {
          message: `Save never wrote ${JSON.stringify(content)}.`,
        });
        await titleBar.waitForDirty(false);
      }

      expectNoNewFiles(externalRoot, seeded.externalBefore);
      expectNoNewFiles(notesRoot, seeded.slatesBefore);
    },
  );

  scenario(
    "file.guard.save",
    "saves through the switch guard before opening a new slate",
    async () => {
      const seeded = await seedLocalFile("switch-guard.txt");
      const expected = "local lifecycle: save from the switch guard";

      await editor.replaceText(expected);
      await titleBar.waitForDirty(true);

      await app.requestNewSlate();
      await dialogs.unsavedChanges.waitForOpen();
      await dialogs.unsavedChanges.save();
      await dialogs.unsavedChanges.waitForClosed();

      await waitForFile(seeded.localPath, (content) => content === expected, {
        message: "The switch guard's Save never reached disk.",
      });
      await editor.waitUntilReady({
        documentPath: "New Slate",
        documentLength: 0,
        timeoutMs: TIMEOUTS.editor,
      });

      // Saving on the way out must not also leave the edit behind as a slate.
      expectNoNewFiles(externalRoot, seeded.externalBefore);
      expectNoNewFiles(notesRoot, seeded.slatesBefore);
    },
  );

  scenario(
    "file.identity.local-lifecycle",
    "keeps one path across reopen and a save taken from the close guard",
    async () => {
      const seeded = await seedLocalFile("close-guard.txt");
      const expected = "local lifecycle: save from the close guard";

      // Reopen the same path so identity is asserted across a full close/open
      // cycle rather than only within a single mount.
      await openPath(seeded.localPath);
      await editor.replaceText(expected);
      await titleBar.waitForDirty(true);

      await titleBar.closeWindow();
      await dialogs.unsavedChanges.waitForOpen();
      await dialogs.unsavedChanges.save();

      // The window is being destroyed, so there is no WebDriver session left to
      // poll through; read the disk directly.
      await waitForFileWithoutWebDriver(
        seeded.localPath,
        (content) => content === expected,
        { message: `The close guard's Save never updated ${seeded.localPath}.` },
      );

      expect(fs.existsSync(path.join(externalRoot, seeded.fileName))).toBe(true);
      expectNoNewFiles(externalRoot, seeded.externalBefore);
      expectNoNewFiles(notesRoot, seeded.slatesBefore);
    },
    { completion: "window-closed" },
  );
});
