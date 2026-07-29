import fs from "node:fs";
import { expect } from "@wdio/globals";
import { TIMEOUTS } from "../config/timeouts.js";
import { expectFileBytes } from "../assertions/matchers.js";
import { scenario } from "../coverage/scenario.js";
import { waitFor, waitForFileBytes } from "../driver/wait.js";
import {
  openBytes,
  openBytesExpectingPrompt,
  openPath,
  provisionSlate,
} from "../fixtures/factories.js";
import { clickTestId, isAriaDisabled, textOf } from "../pages/common.js";
import * as dialogs from "../pages/dialogs.js";
import * as editor from "../pages/editor.js";
import * as app from "../pages/app.js";
import * as settings from "../pages/settings.js";
import * as statusBar from "../pages/statusBar.js";
import * as titleBar from "../pages/titleBar.js";
import * as transformations from "../pages/transformations.js";

/**
 * Character encoding.
 *
 * Every assertion is on raw bytes. A decoded-string comparison cannot tell a
 * UTF-8 `é` (C3 A9) from a Windows-1252 `é` (E9), which is exactly the
 * difference these tests exist to protect.
 */
const BOM = Buffer.from([0xef, 0xbb, 0xbf]);

/** "café" as Windows-1252: the é is a single 0xE9 byte, not a UTF-8 pair. */
const CAFE_1252 = Buffer.from([0x63, 0x61, 0x66, 0xe9]);

/** The active document's path, once autosave has given the slate one. */
async function currentDocumentPath(): Promise<string> {
  let path: string | null = null;
  await waitFor(
    async () => {
      path = await titleBar.documentPath();
      return Boolean(path && path !== "New Slate");
    },
    {
      message: () => `The slate never received a file path. Last seen: ${JSON.stringify(path)}`,
      timeoutMs: TIMEOUTS.disk,
    },
  );
  return path as unknown as string;
}

describe("Character encoding", () => {
  scenario(
    "format.encoding.utf8-bom",
    "detects a UTF-8 BOM without exposing it as editor text",
    async () => {
      const original = Buffer.concat([BOM, Buffer.from("alpha\nbeta\n", "utf8")]);
      const filePath = await openBytes("utf8-bom.txt", original);

      await statusBar.waitForEncoding("utf-8-bom");
      // The BOM is metadata: it must not appear in the document or its length.
      expect((await editor.text()).trimEnd()).toBe("alpha\nbeta");
      expect(await statusBar.documentLength()).toBe(String("alpha\nbeta\n".length));
      expectFileBytes(filePath, original);
    },
  );

  scenario(
    "format.encoding.utf16le",
    "detects UTF-16 LE and converts it explicitly to UTF-8",
    async () => {
      const text = "alpha\nβeta\n";
      const filePath = await openBytes(
        "utf16-le.txt",
        Buffer.from(`﻿${text}`, "utf16le"),
      );

      await statusBar.waitForEncoding("utf-16le");
      await statusBar.saveWithEncoding("utf-8");
      expectFileBytes(filePath, Buffer.from(text, "utf8"));
    },
  );

  scenario(
    "format.encoding.utf16be",
    "detects UTF-16 BE and converts it to UTF-8 on request",
    async () => {
      const text = "alpha\nβeta\n";
      // Node has no utf16be encoder; build LE then swap each byte pair.
      const body = Buffer.from(`﻿${text}`, "utf16le");
      body.swap16();
      const filePath = await openBytes("utf16-be.txt", body);

      await statusBar.waitForEncoding("utf-16be");
      expect((await editor.text()).trimEnd()).toBe("alpha\nβeta");

      await statusBar.saveWithEncoding("utf-8");
      expectFileBytes(filePath, Buffer.from(text, "utf8"));
    },
  );

  scenario(
    "format.encoding.windows1252-prompt",
    "asks before interpreting invalid UTF-8 as Windows-1252",
    async () => {
      // The read stops to ask before decoding, so do not wait for the
      // document here: it cannot become ready until the prompt is answered.
      const filePath = await openBytesExpectingPrompt("windows-1252.txt", CAFE_1252);

      await dialogs.encodingConfirmation.waitForOpen();
      await dialogs.encodingConfirmation.accept();

      await statusBar.waitForEncoding("windows-1252");
      await editor.waitForExactText("café");
      // Reading must never rewrite the file.
      expectFileBytes(filePath, CAFE_1252);
    },
  );

  scenario(
    "format.encoding.lossy-save-refused",
    "refuses a lossy Windows-1252 save and keeps the original bytes",
    async () => {
      const filePath = await openBytesExpectingPrompt("windows-1252-loss.txt", CAFE_1252);
      await dialogs.encodingConfirmation.waitForOpen();
      await dialogs.encodingConfirmation.accept();
      await statusBar.waitForEncoding("windows-1252");

      // β has no Windows-1252 representation, so this save must be refused
      // rather than silently substituting a replacement character.
      await editor.replaceText("café β");
      await titleBar.waitForDirty(true);
      await editor.save();

      await transformations.waitForToastContaining("cannot be represented");
      expectFileBytes(filePath, CAFE_1252);
      // Refusing must leave the work unsaved, not pretend it succeeded.
      await titleBar.waitForDirty(true);

      // Recovery is explicit and lossless.
      await statusBar.saveWithEncoding("utf-8");
      expectFileBytes(filePath, Buffer.from("café β", "utf8"));
      await titleBar.waitForDirty(false);
    },
  );

  scenario(
    "format.encoding.reopen-saves-dirty-local",
    "saves a dirty local file before reopening it in another encoding",
    async () => {
      const beforeReopen = "alpha\nsaved before reopen\n";
      const afterReopen = "alpha\nsaved before reopen\ncafé\n";
      const filePath = await openBytes(
        "local-reopen-lifecycle.txt",
        Buffer.from("alpha\n", "utf8"),
      );

      await statusBar.waitForEncoding("utf-8");
      await titleBar.waitForDirty(false);
      await titleBar.waitForSaveEnabled(false);

      await editor.replaceText(beforeReopen);
      await titleBar.waitForDirty(true);

      await clickTestId("status-encoding");
      await clickTestId("encoding-select-trigger");
      await clickTestId("encoding-item-windows-1252");

      // Reopening a dirty document is offered, not blocked: the guard handles
      // the unsaved work instead of the picker refusing up front.
      expect(await isAriaDisabled("encoding-reopen")).toBe(false);
      expect(await textOf("encoding-picker-dialog")).not.toContain(
        "Save or discard your changes before reopening.",
      );

      await clickTestId("encoding-reopen");
      await dialogs.unsavedChanges.waitForOpen();
      // The pending edit must still be intact while the guard is up.
      expect(await editor.text()).toBe(beforeReopen);

      await dialogs.unsavedChanges.save();
      await dialogs.unsavedChanges.waitForClosed();

      await statusBar.waitForEncoding("windows-1252");
      await editor.waitForExactText(beforeReopen);
      await waitForFileBytes(filePath, (bytes) => bytes.equals(Buffer.from(beforeReopen, "utf8")), {
        message: "The guard's Save did not write the pre-reopen content.",
      });
      await titleBar.waitForDirty(false);

      // A later edit must now be written in the newly chosen encoding.
      await editor.replaceText(afterReopen);
      await titleBar.waitForDirty(true);
      await editor.save();

      const expected = Buffer.concat([
        Buffer.from("alpha\nsaved before reopen\ncaf", "ascii"),
        Buffer.from([0xe9]),
        Buffer.from("\n", "ascii"),
      ]);
      await waitForFileBytes(filePath, (bytes) => bytes.equals(expected), {
        message: "The post-reopen save was not written as Windows-1252.",
      });
      // Negative: no UTF-8 encoding of é survived anywhere in the file.
      expect(fs.readFileSync(filePath).includes(Buffer.from([0xc3, 0xa9]))).toBe(false);
      await titleBar.waitForDirty(false);
    },
  );

  scenario(
    "format.encoding.reopen-slate-silent",
    "silently saves a managed slate before reopen and keeps autosaving in the new encoding",
    async () => {
      const beforeReopen = "slate\nsaved before reopen\n";
      const afterReopen = "slate\nsaved before reopen\ncafé\n";
      const filePath = provisionSlate("slate-reopen-lifecycle.txt", "slate\n");

      const descriptor = await openPath(filePath);
      expect(descriptor.source).toBe("slates");
      await statusBar.waitForEncoding("utf-8");
      await editor.replaceText(beforeReopen);

      await statusBar.reopenWithEncoding("windows-1252");
      // A slate is never dirty, so the guard must not appear at all.
      expect(await dialogs.unsavedChanges.isOpen()).toBe(false);

      await statusBar.waitForEncoding("windows-1252");
      await editor.waitForExactText(beforeReopen);
      await waitForFileBytes(filePath, (bytes) => bytes.equals(Buffer.from(beforeReopen, "utf8")), {
        message: "The slate was not flushed before reopening.",
      });

      // Autosave must keep working, and in the new encoding.
      await editor.replaceText(afterReopen);
      const expected = Buffer.concat([
        Buffer.from("slate\nsaved before reopen\ncaf", "ascii"),
        Buffer.from([0xe9]),
        Buffer.from("\n", "ascii"),
      ]);
      await waitForFileBytes(filePath, (bytes) => bytes.equals(expected), {
        message: "Autosave did not continue in the reopened encoding.",
        timeoutMs: TIMEOUTS.disk,
      });
    },
  );
  scenario(
    "format.encoding.default-for-new",
    "applies the default encoding setting to a newly created slate",
    async () => {
      await settings.open();
      await settings.setCharacterEncoding("utf-8-bom");
      await settings.close();

      await app.newSlate();
      await statusBar.waitForEncoding("utf-8-bom");

      // Prove it reaches disk rather than only the status bar: a BOM is three
      // bytes at the front of the autosaved file.
      await editor.replaceText("default encoding probe\n");
      const slatePath = await currentDocumentPath();
      await waitForFileBytes(slatePath, (bytes) => bytes.subarray(0, 3).equals(BOM), {
        message: "The new slate was not autosaved with the configured BOM.",
        timeoutMs: TIMEOUTS.disk,
      });

      await settings.open();
      await settings.setCharacterEncoding("utf-8");
      await settings.close();
    },
  );

  scenario(
    "format.eol.default-for-new",
    "applies the default line-ending setting to a newly created slate",
    async () => {
      await settings.open();
      await settings.setLineEnding("crlf");
      await settings.close();

      await app.newSlate();
      await statusBar.waitForEol("crlf");

      await editor.replaceText("first\nsecond\n");
      const slatePath = await currentDocumentPath();
      await waitForFileBytes(
        slatePath,
        (bytes) => bytes.equals(Buffer.from("first\r\nsecond\r\n", "utf8")),
        {
          message: "The new slate was not autosaved with the configured line ending.",
          timeoutMs: TIMEOUTS.disk,
        },
      );

      await settings.open();
      await settings.setLineEnding("lf");
      await settings.close();
    },
  );
});
