import fs from "node:fs";
import path from "node:path";
import { expect } from "@wdio/globals";
import { TIMEOUTS } from "../config/timeouts.js";
import { scenario } from "../coverage/scenario.js";
import { waitForExternalAction } from "../driver/externalAction.js";
import {
  armOperationGate,
  releaseOperationGate,
  waitForOperationGate,
} from "../driver/operationGate.js";
import { waitFor } from "../driver/wait.js";
import {
  externalRoot,
  notesRoot,
  openPath,
  openText,
  provisionSlate,
  provisionText,
} from "../fixtures/factories.js";
import { pressMod } from "../driver/keys.js";
import { byTestId } from "../pages/common.js";
import * as dialogs from "../pages/dialogs.js";
import * as editor from "../pages/editor.js";
import * as sidebar from "../pages/sidebar.js";
import * as settings from "../pages/settings.js";
import { waitForClipboardText } from "../driver/clipboard.js";

/**
 * The library sidebar.
 *
 * Search and sorting are backend-driven, and the list deliberately resists
 * reordering under the cursor, so the assertions here are about observable
 * order and membership rather than about any single control's state.
 */
describe("Library sidebar", () => {
  /**
   * Seed files into the library.
   *
   * Writing to the notes root is not enough: the sidebar lists *tracked* recent
   * files from the backend registry, not a directory listing, so a file only
   * appears once it has been opened. Seeding by opening is also what gives the
   * recency ordering something real to sort by.
   */
  async function seedLibraryFiles(
    files: { name: string; body: string }[],
  ): Promise<string[]> {
    const paths: string[] = [];
    for (const file of files) {
      const target = provisionSlate(file.name, file.body);
      await openPath(target);
      paths.push(target);
    }
    await sidebar.ensureOpen();
    await sidebar.setFilterTab("unified");
    await sidebar.waitForPaths(
      (visible) => paths.every((file) => visible.includes(file)),
      "The seeded files never appeared in the sidebar.",
    );
    return paths;
  }

  /** Three files with distinct names and sizes, for ordering assertions. */
  async function seedSortable(): Promise<string[]> {
    return seedLibraryFiles([
      { name: "alpha-sort.txt", body: "a".repeat(64) },
      { name: "beta-sort.txt", body: "b".repeat(256) },
      { name: "gamma-sort.txt", body: "c".repeat(1024) },
    ]);
  }

  scenario("sidebar.sort.name", "orders the list by name in both directions", async () => {
    await seedSortable();

    await sidebar.setSort("name-asc");
    await sidebar.waitForPaths((paths) => {
      const names = paths.map((file) => path.basename(file));
      return names.join() === [...names].sort().join();
    }, "Ascending name sort did not order the list.");

    await sidebar.setSort("name-desc");
    await sidebar.waitForPaths((paths) => {
      const names = paths.map((file) => path.basename(file));
      return names.join() === [...names].sort().reverse().join();
    }, "Descending name sort did not order the list.");
  });

  scenario("sidebar.sort.size", "orders the list by size in both directions", async () => {
    await seedSortable();

    const sizeOf = (file: string): number => fs.statSync(file).size;

    await sidebar.setSort("size-desc");
    await sidebar.waitForPaths((paths) => {
      const sizes = paths.filter((f) => fs.existsSync(f)).map(sizeOf);
      return sizes.every((size, index) => index === 0 || sizes[index - 1]! >= size);
    }, "Largest-first sort did not order the list by size.");

    await sidebar.setSort("size-asc");
    await sidebar.waitForPaths((paths) => {
      const sizes = paths.filter((f) => fs.existsSync(f)).map(sizeOf);
      return sizes.every((size, index) => index === 0 || sizes[index - 1]! <= size);
    }, "Smallest-first sort did not order the list by size.");
  });

  scenario(
    "sidebar.sort.recency",
    "orders the list by how recently each file was opened",
    async () => {
      const seeded = await seedSortable();
      const mostRecent = seeded.at(-1)!;
      const leastRecent = seeded[0]!;

      await sidebar.setSort("recently-opened");
      await sidebar.waitForPaths(
        (paths) => paths.indexOf(mostRecent) < paths.indexOf(leastRecent),
        "Most-recently-opened did not put the newest file first.",
      );

      await sidebar.setSort("least-recently-opened");
      await sidebar.waitForPaths(
        (paths) => paths.indexOf(leastRecent) < paths.indexOf(mostRecent),
        "Least-recently-opened did not put the oldest file first.",
      );
    },
  );

  scenario(
    "sidebar.search.text",
    "finds files by content through the Rust backend",
    async () => {
      const needle = "distinctive-search-token";
      const [match] = await seedLibraryFiles([
        { name: "search-hit.txt", body: `line one\n${needle}\nline three\n` },
        { name: "search-miss.txt", body: "nothing relevant here\n" },
      ]);

      await sidebar.search(needle);
      await sidebar.waitForPaths(
        (paths) => paths.includes(match) && paths.length === 1,
        `Searching for '${needle}' did not return only the matching file.`,
      );

      await sidebar.clearSearch();
      await sidebar.waitForPaths(
        (paths) => paths.length > 1,
        "Clearing the search did not restore the full list.",
      );
    },
  );

  scenario(
    "sidebar.search.empty-state",
    "shows nothing when a query matches no file",
    async () => {
      await seedSortable();

      await sidebar.search("zzz-no-file-contains-this-token");
      await sidebar.waitForPaths(
        (paths) => paths.length === 0,
        "A query with no matches still listed files.",
      );

      await sidebar.clearSearch();
      await sidebar.waitForPaths(
        (paths) => paths.length > 0,
        "Clearing an empty search did not restore the list.",
      );
    },
  );

  scenario(
    "sidebar.search.clear",
    "resets the query and its modifiers when the search is cleared",
    async () => {
      await seedSortable();

      await sidebar.search("alpha");
      await sidebar.toggleSearchOption("case");
      await waitFor(async () => sidebar.searchOptionPressed("case"), {
        message: "The case-sensitivity toggle never became active.",
      });

      await sidebar.clearSearch();
      await waitFor(
        async () =>
          (await sidebar.searchValue()) === "" && !(await sidebar.searchOptionPressed("case")),
        { message: "Clearing the search did not reset both the query and its modifiers." },
      );
    },
  );

  scenario(
    "file.rename.applies",
    "renames a slate on disk and in the library",
    async () => {
      const [original] = await seedLibraryFiles([
        { name: "rename-me.txt", body: "rename target\n" },
      ]);

      await sidebar.cardAction(original, "rename");
      await dialogs.renameFile.waitForOpen();
      await dialogs.renameFile.setName("renamed-by-e2e.txt");
      await dialogs.renameFile.submit();
      await dialogs.renameFile.waitForClosed();

      const renamed = path.join(notesRoot, "renamed-by-e2e.txt");
      await waitFor(() => fs.existsSync(renamed) && !fs.existsSync(original), {
        message: "Rename did not move the file on disk.",
        timeoutMs: TIMEOUTS.disk,
      });
      await sidebar.waitForCard(renamed);
      expect(fs.readFileSync(renamed, "utf8")).toBe("rename target\n");
    },
  );

  scenario(
    "file.rename.validation",
    "rejects an empty name and one containing a path separator",
    async () => {
      const [original] = await seedLibraryFiles([
        { name: "rename-validate.txt", body: "validate\n" },
      ]);

      await sidebar.cardAction(original, "rename");
      await dialogs.renameFile.waitForOpen();

      await dialogs.renameFile.setName("");
      await dialogs.renameFile.submit();
      // Still open: an empty name is refused rather than silently accepted.
      expect(await dialogs.renameFile.isOpen()).toBe(true);

      await dialogs.renameFile.setName("nested/escape.txt");
      await dialogs.renameFile.submit();
      expect(await dialogs.renameFile.isOpen()).toBe(true);
      // And nothing escaped the notes root.
      expect(fs.existsSync(path.join(notesRoot, "nested"))).toBe(false);

      // Restore a valid name so the dialog can be dismissed normally.
      await dialogs.renameFile.setName("rename-validate.txt");
      await dialogs.renameFile.cancel();
      expect(fs.existsSync(original)).toBe(true);
    },
  );

  scenario("file.duplicate", "duplicates a slate into a second distinct file", async () => {
    const [original] = await seedLibraryFiles([
      { name: "duplicate-me.txt", body: "duplicate body\n" },
    ]);

    await sidebar.cardAction(original, "duplicate");

    await waitFor(
      () => {
        const copies = fs
          .readdirSync(notesRoot)
          .map((name) => path.join(notesRoot, name))
          .filter(
            (candidate) =>
              candidate !== original &&
              fs.readFileSync(candidate, "utf8") === "duplicate body\n",
          );
        return copies.length === 1;
      },
      {
        message: "Duplicating did not create exactly one copy.",
        timeoutMs: TIMEOUTS.disk,
      },
    );
    // The original survives duplication.
    expect(fs.existsSync(original)).toBe(true);
  });

  scenario(
    "file.duplicate-as-slate",
    "copies a local file into the notes root as a managed slate",
    async () => {
      const local = provisionText("promote-me.txt", "promote body\n");
      await openPath(local);
      await sidebar.ensureOpen();
      await sidebar.setFilterTab("unified");
      await sidebar.waitForCard(local);

      await sidebar.cardAction(local, "duplicate-as-slate");

      await waitFor(
        () =>
          fs
            .readdirSync(notesRoot)
            .some(
              (name) =>
                fs.readFileSync(path.join(notesRoot, name), "utf8") === "promote body\n",
            ),
        {
          message: "Duplicate-as-slate did not copy the file into the notes root.",
          timeoutMs: TIMEOUTS.disk,
        },
      );
      // The external original is untouched.
      expect(fs.readFileSync(local, "utf8")).toBe("promote body\n");
    },
  );

  scenario(
    "file.unlink",
    "untracks a local file from the library without deleting it",
    async () => {
      const local = provisionText("untrack-me.txt", "untrack body\n");
      await openPath(local);
      await sidebar.ensureOpen();
      await sidebar.setFilterTab("unified");
      await sidebar.waitForCard(local);

      // Unlink has no delete confirmation of its own: it removes the file from
      // the library only, so there is nothing destructive to confirm.
      await sidebar.cardAction(local, "unlink");

      await sidebar.waitForCard(local, false);
      expect(fs.existsSync(local)).toBe(true);
      expect(fs.readFileSync(local, "utf8")).toBe("untrack body\n");
    },
  );

  scenario(
    "file.delete.confirmed",
    "asks before deleting, then removes the file and its card",
    async () => {
      const [doomed] = await seedLibraryFiles([
        { name: "delete-me.txt", body: "delete body\n" },
      ]);

      await sidebar.cardAction(doomed, "delete");
      await dialogs.deleteFile.waitForOpen();

      // Cancelling must keep the file.
      await dialogs.deleteFile.cancel();
      expect(fs.existsSync(doomed)).toBe(true);

      await sidebar.cardAction(doomed, "delete");
      await dialogs.deleteFile.waitForOpen();
      await dialogs.deleteFile.confirm();

      await waitFor(() => !fs.existsSync(doomed), {
        message: "Confirming delete did not remove the file.",
        timeoutMs: TIMEOUTS.disk,
      });
      await sidebar.waitForCard(doomed, false);
    },
  );

  scenario(
    "file.delete.without-confirmation",
    "deletes immediately when the confirmation setting is turned off",
    async () => {
      const [doomed] = await seedLibraryFiles([
        { name: "delete-unconfirmed.txt", body: "no prompt\n" },
      ]);

      await settings.open();
      await settings.setConfirmBeforeDelete(false);
      await settings.close();

      try {
        await sidebar.cardAction(doomed, "delete");
        await waitFor(() => !fs.existsSync(doomed), {
          message: "Delete without confirmation did not remove the file.",
          timeoutMs: TIMEOUTS.disk,
        });
        expect(await dialogs.deleteFile.isOpen()).toBe(false);
      } finally {
        // Restore the global default so later scenarios see the shipped setting.
        await settings.open();
        await settings.setConfirmBeforeDelete(true);
        await settings.close();
      }
    },
  );

  scenario(
    "file.copy-path",
    "copies the document's full path to the clipboard",
    async () => {
      const [target] = await seedLibraryFiles([
        { name: "copy-path.txt", body: "copy path body\n" },
      ]);

      await sidebar.cardAction(target, "copy-path");
      // Assert the clipboard itself rather than the toast copy: the message
      // wording is presentation, the path on the clipboard is the behavior.
      await waitForClipboardText(target!);
    },
  );

  scenario(
    "file.reveal",
    "hands the validated file path to the OS reveal boundary",
    async () => {
      const [target] = await seedLibraryFiles([
        { name: "reveal-target.txt", body: "reveal target\n" },
      ]);

      await sidebar.cardAction(target, "reveal");
      expect(await waitForExternalAction()).toEqual({
        kind: "reveal",
        target,
      });
    },
  );

  scenario("sidebar.find-files-shortcut", "focuses the search input", async () => {
    await sidebar.ensureOpen();
    await pressMod("p");
    const input = await byTestId("sidebar-search-input");
    await waitFor(async () => input.isFocused(), {
      message: "The Find Files shortcut did not focus the sidebar search input.",
    });
  });

  scenario("sidebar.toggle", "collapses and restores the sidebar", async () => {
    await sidebar.ensureOpen();
    expect(await sidebar.isOpen()).toBe(true);

    await sidebar.ensureClosed();
    expect(await sidebar.isOpen()).toBe(false);

    await sidebar.ensureOpen();
    expect(await sidebar.isOpen()).toBe(true);
  });

  scenario(
    "sidebar.reorder-suppression",
    "does not reorder the list under the cursor when a file is opened from it",
    async () => {
      await seedSortable();
      await sidebar.setSort("recently-opened");

      const before = await sidebar.visiblePaths();
      const target = before.at(-1);
      expect(target).toBeTruthy();

      // Opening from the sidebar makes this the most recent file, which under a
      // recency sort would normally jump it to the top — directly under the
      // pointer the user is still holding. The list must stay put instead.
      await sidebar.openCard(target!);
      await editor.waitUntilReady({ documentPath: target! });

      const after = await sidebar.visiblePaths();
      expect(after).toEqual(before);
    },
  );

  scenario(
    "sidebar.refresh-on-mutation",
    "refreshes the list from backend file mutations without a manual reload",
    async () => {
      const [target] = await seedLibraryFiles([
        { name: "refresh-source.txt", body: "refresh body\n" },
      ]);

      // Renaming emits a backend event; the list must follow it on its own.
      await sidebar.cardAction(target, "rename");
      await dialogs.renameFile.waitForOpen();
      await dialogs.renameFile.setName("refresh-renamed.txt");
      await dialogs.renameFile.submit();
      await dialogs.renameFile.waitForClosed();

      const renamed = path.join(notesRoot, "refresh-renamed.txt");
      // No sidebar.refresh() here on purpose.
      await sidebar.waitForCard(renamed);
      await sidebar.waitForCard(target, false);
    },
  );

  scenario(
    "sidebar.search.modifiers",
    "changes results for case-sensitive, whole-word, and regex searches",
    async () => {
      const [upper, lower] = await seedLibraryFiles([
        { name: "case-upper.txt", body: "TOKENCASE marker\n" },
        { name: "case-lower.txt", body: "tokencase marker\n" },
      ]);

      try {
        // Case-insensitive by default: both files match.
        await sidebar.search("tokencase");
        await sidebar.waitForPaths(
          (paths) => paths.includes(upper) && paths.includes(lower),
          "A case-insensitive search did not match both files.",
        );

        await sidebar.toggleSearchOption("case");
        await sidebar.waitForPaths(
          (paths) => paths.includes(lower) && !paths.includes(upper),
          "Enabling case sensitivity did not narrow the results.",
        );

        await sidebar.toggleSearchOption("case");
        await sidebar.clearSearch();
        await sidebar.search("TOKEN");
        await sidebar.toggleSearchOption("word");
        await sidebar.waitForPaths(
          (paths) => paths.length === 0,
          "Whole-word search incorrectly matched TOKEN inside TOKENCASE.",
        );

        await sidebar.clearSearch();
        await sidebar.search("TOKEN(?:CASE|MISS)");
        await sidebar.toggleSearchOption("case");
        await sidebar.toggleSearchOption("regex");
        await sidebar.waitForPaths(
          (paths) => paths.includes(upper) && paths.length === 1,
          "Case-sensitive regex search did not return only the uppercase file.",
        );
      } finally {
        if ((await sidebar.searchValue()) !== "") await sidebar.clearSearch();
        for (const option of ["case", "word", "regex"] as const) {
          if (await sidebar.searchOptionPressed(option)) {
            await sidebar.toggleSearchOption(option);
          }
        }
      }
    },
  );

  scenario(
    "sidebar.search.cancel",
    "cancels an in-flight backend search when the query is cleared",
    async () => {
      const [needleFile, otherFile] = await seedLibraryFiles([
        { name: "cancel-search-a.txt", body: `${"needle line\n".repeat(10_000)}` },
        { name: "cancel-search-b.txt", body: `${"other line\n".repeat(10_000)}` },
      ]);
      await armOperationGate("sidebar-search");
      await sidebar.search("needle");
      await waitForOperationGate("sidebar-search");

      try {
        await sidebar.clearSearch();
      } finally {
        await releaseOperationGate("sidebar-search");
      }

      await waitFor(async () => (await sidebar.searchValue()) === "", {
        message: "Clearing a running search did not reset the input.",
      });
      await sidebar.waitForPaths(
        (paths) => paths.includes(needleFile) && paths.includes(otherFile),
        "Cancelling search did not restore the recent-files list.",
      );
    },
  );
});
