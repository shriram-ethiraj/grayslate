import fs from "node:fs";
import path from "node:path";
import { expect } from "@wdio/globals";
import { TIMEOUTS } from "../config/timeouts.js";
import {
  directoryInventory,
  expectNoNewFiles,
  expectSettledAbsent,
} from "../assertions/matchers.js";
import { scenario } from "../coverage/scenario.js";
import { waitFor, waitForFile, waitForFileWithoutWebDriver } from "../driver/wait.js";
import { notesRoot, openPath } from "../fixtures/factories.js";
import * as app from "../pages/app.js";
import * as dialogs from "../pages/dialogs.js";
import * as editor from "../pages/editor.js";
import * as titleBar from "../pages/titleBar.js";

/**
 * The slate save contract.
 *
 * A slate is backend-autosaved and never dirty, so the property that matters is
 * *identity*: once the first autosave has named the file, every later write —
 * autosave, manual save, a document switch, a window close — must land on that
 * same path rather than minting a second content-named slate.
 *
 * The original regression this guards: an edit after the first autosave forked
 * a new file because the save path re-derived the name from content.
 */
describe("Slate save lifecycle", () => {
  /**
   * Start from a fresh slate, type `content`, and return the file autosave
   * created for it.
   *
   * The `newSlate()` is not incidental: without it a scenario would inherit
   * whichever document a sibling left open, and would then be asserting against
   * that document's identity rather than one it established itself. Slates are
   * never dirty, so this never raises the unsaved-changes guard.
   */
  async function seedSlate(content: string): Promise<{ slatePath: string; before: string[] }> {
    await app.newSlate();
    await editor.replaceText(content);

    let slatePath = "";
    await waitFor(
      () => {
        const matches = directoryInventory(notesRoot)
          .map((name) => path.join(notesRoot, name))
          .filter((candidate) => fs.readFileSync(candidate, "utf8") === content);
        if (matches.length !== 1) return false;
        slatePath = matches[0] ?? "";
        return slatePath !== "";
      },
      {
        message: `Autosave did not create exactly one file containing ${JSON.stringify(content)}.`,
        timeoutMs: TIMEOUTS.disk,
      },
    );

    await editor.waitUntilReady({
      documentPath: slatePath,
      documentLength: content.length,
    });
    return { slatePath, before: directoryInventory(notesRoot) };
  }

  /**
   * Wait until the file the editor currently points at holds `content`.
   *
   * Reads the path from the title bar rather than assuming the one autosave
   * first chose, because slate names are derived from content and may legally
   * change with it.
   */
  async function waitForCurrentDocumentContent(content: string): Promise<void> {
    let observedPath: string | null = null;
    await waitFor(
      async () => {
        observedPath = await titleBar.documentPath();
        if (!observedPath || !fs.existsSync(observedPath)) return false;
        return fs.readFileSync(observedPath, "utf8") === content;
      },
      {
        message:
          `The active slate never held ${JSON.stringify(content.slice(0, 60))}. ` +
          `Last path: ${observedPath}`,
        timeoutMs: TIMEOUTS.disk,
      },
    );
  }

  scenario(
    "file.slate.autosave-and-name",
    "creates exactly one content-named file on the first autosave",
    async () => {
      const content = "slate lifecycle: first autosave";
      const { slatePath } = await seedSlate(content);

      expect(path.dirname(slatePath)).toBe(notesRoot);
      expect(fs.readFileSync(slatePath, "utf8")).toBe(content);
      // Exactly one file, not one-per-edit.
      expect(directoryInventory(notesRoot)).toHaveLength(1);
    },
  );

  scenario(
    "file.identity.slate-lifecycle",
    "keeps one file identity across a second autosave and repeated manual saves",
    async () => {
      const { slatePath, before } = await seedSlate("slate lifecycle: identity base");

      // The original regression: a further edit must land in the document the
      // first autosave established, not fork a second content-named slate.
      //
      // The assertion is "no new file", not "the filename never changes".
      // Naming is content-derived, so a rename is legitimate; a *second* file
      // is the actual defect, and it is what the bug produced.
      const second = "slate lifecycle: second autosave updates the same file";
      await editor.replaceText(second);
      await waitForCurrentDocumentContent(second);
      expectNoNewFiles(notesRoot, before);
      expect(directoryInventory(notesRoot)).toHaveLength(before.length);

      const third = "slate lifecycle: repeated manual saves update the same file";
      await editor.replaceText(third);
      await editor.save();
      await editor.save();
      await editor.save();
      await waitForCurrentDocumentContent(third);

      // The old version slept 2.5 s before checking that no second slate had
      // appeared, which made the assertion weaker on slower machines. Hold the
      // invariant across a sampled window instead, starting from a point where
      // the write has demonstrably landed.
      await expectSettledAbsent({
        precondition: async () => {
          await waitForFile(slatePath, (content) => content === third, {
            message: "The manual save never reached disk.",
          });
        },
        invariant: async () => directoryInventory(notesRoot).length === before.length,
        message: "Autosave and manual save together must never fork a second slate.",
        quietForMs: 4_000,
      });
    },
  );

  scenario(
    "file.slate.switch-flushes",
    "flushes to the same file when switching away and reopening",
    async () => {
      const { slatePath, before } = await seedSlate("slate lifecycle: switch base");

      const switched = "slate lifecycle: switching flushes the same file";
      await editor.replaceText(switched);
      await app.newSlate();

      await waitForFile(slatePath, (content) => content === switched, {
        message: "Switching documents did not flush the pending edit to the slate.",
      });
      // A fresh untitled slate carries no content, so it is not written to disk
      // yet: the switch must not add a file.
      expectNoNewFiles(notesRoot, before);

      await openPath(slatePath);
      await editor.waitUntilReady({
        documentPath: slatePath,
        documentLength: switched.length,
      });
      expect(fs.readFileSync(slatePath, "utf8")).toBe(switched);
    },
  );

  scenario(
    "file.slate.close-flushes",
    "flushes the slate on window close without prompting",
    async () => {
      const { slatePath } = await seedSlate("slate lifecycle: close base");

      const closed = "slate lifecycle: closing flushes the same file";
      await editor.replaceText(closed);
      await titleBar.closeWindow();

      // A slate is never dirty, so closing must not raise the guard at all.
      expect(await dialogs.unsavedChanges.isOpen()).toBe(false);

      // The window is destroyed after the backend flush, so poll the disk
      // directly rather than through a session that is going away.
      await waitForFileWithoutWebDriver(slatePath, (content) => content === closed, {
        message: `Closing did not flush the pending edit to ${slatePath}.`,
      });
    },
  );
});
