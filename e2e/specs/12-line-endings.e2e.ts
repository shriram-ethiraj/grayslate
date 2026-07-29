import fs from "node:fs";
import { expect } from "@wdio/globals";
import { expectFileBytes } from "../assertions/matchers.js";
import { scenario } from "../coverage/scenario.js";
import { waitForFile } from "../driver/wait.js";
import { openPath, openText, provisionSlate } from "../fixtures/factories.js";
import { waitForClipboardText } from "../driver/clipboard.js";
import { byTestId, clickTestId, existsTestId, pressEscape } from "../pages/common.js";
import * as csv from "../pages/csv.js";
import * as editor from "../pages/editor.js";
import * as statusBar from "../pages/statusBar.js";
import * as titleBar from "../pages/titleBar.js";
import { waitForAttribute } from "../driver/wait.js";

/**
 * Line endings.
 *
 * The editor holds every document canonically LF-terminated — CodeMirror
 * enforces it — so the line ending is per-document metadata applied only at the
 * disk-write boundary. Every assertion here is on raw bytes: a dropped `\r` is
 * invisible in a string comparison.
 *
 * Each scenario opens its own file, so all of them are independent.
 */
describe("Line endings", () => {
  scenario(
    "format.eol.detect-crlf",
    "detects CRLF, opens clean, and leaves the file byte-identical",
    async () => {
      const original = "alpha\r\nbeta\r\ngamma\r\n";
      const filePath = await openText("crlf-clean.txt", original);

      await statusBar.waitForEol("crlf");
      // The regression this feature fixed: comparing the editor's normalized
      // text against raw CRLF bytes marked every Windows file dirty on open,
      // then silently rewrote it as LF.
      await titleBar.waitForDirty(false);

      expectFileBytes(filePath, original);
    },
  );

  scenario(
    "format.eol.preserve-crlf-on-save",
    "keeps CRLF when the file is edited and saved",
    async () => {
      const filePath = await openText("crlf-edit.txt", "one\r\ntwo\r\n");
      await statusBar.waitForEol("crlf");

      await editor.replaceText("one\ntwo\nthree\n");
      await editor.save();

      await waitForFile(filePath, (content) => content.includes("three"), {
        message: "The edited CRLF file never reached disk.",
      });
      expectFileBytes(filePath, "one\r\ntwo\r\nthree\r\n");
    },
  );

  scenario(
    "format.eol.legacy-cr",
    "opens legacy CR cleanly and modernizes it to LF on the next write",
    async () => {
      const original = "red\rgreen\r";
      const filePath = await openText("cr-legacy.txt", original);

      await statusBar.waitForEol("lf");
      await titleBar.waitForDirty(false);
      // Untouched until the user writes: opening must not rewrite the file.
      expectFileBytes(filePath, original);

      await editor.replaceText("red\ngreen\nblue\n");
      await editor.save();

      await waitForFile(filePath, (content) => content.includes("blue"), {
        message: "The rewritten CR file never reached disk.",
      });
      expectFileBytes(filePath, "red\ngreen\nblue\n");
    },
  );

  scenario(
    "format.eol.mixed-dominant",
    "uses the dominant style for a mixed file and normalizes on save",
    async () => {
      // Three CRLF against one lone LF: CRLF dominates.
      const filePath = await openText("mixed.txt", "a\r\nb\r\nc\r\nd\ne");
      await statusBar.waitForEol("crlf");

      // A real edit is required: retyping the normalized original is a no-op
      // and would never dirty the document or trigger a write.
      await editor.replaceText("a\nb\nc\nd\ne\nf");
      await editor.save();

      await waitForFile(filePath, (content) => content.includes("f"), {
        message: "The mixed-EOL file never saved.",
      });
      // Every break, including the previously lone LF, is now CRLF.
      expectFileBytes(filePath, "a\r\nb\r\nc\r\nd\r\ne\r\nf");
    },
  );

  scenario(
    "format.eol.switch-marks-dirty",
    "marks the document dirty on an EOL switch and converts on save",
    async () => {
      const filePath = await openText("switch.txt", "x\r\ny\r\n");
      await statusBar.waitForEol("crlf");
      await titleBar.waitForDirty(false);

      // Changing the line ending is a real, savable change even though the
      // canonical text is untouched.
      await statusBar.selectEol("lf");
      await titleBar.waitForDirty(true);

      await editor.save();
      await waitForFile(filePath, (content) => !content.includes("\r"), {
        message: "The EOL conversion never reached disk.",
      });
      expectFileBytes(filePath, "x\ny\n");
      await titleBar.waitForDirty(false);
    },
  );

  scenario(
    "format.eol.revert-clears-dirty",
    "clears dirty state when the EOL is switched back before saving",
    async () => {
      await openText("revert.txt", "p\r\nq\r\n");
      await statusBar.waitForEol("crlf");
      await titleBar.waitForDirty(false);

      await statusBar.selectEol("lf");
      await titleBar.waitForDirty(true);

      await statusBar.selectEol("crlf");
      await titleBar.waitForDirty(false);
    },
  );

  scenario(
    "format.eol.picker-state",
    "shows the active value, offers only LF and CRLF, and stays open after applying",
    async () => {
      await openText("picker-dropdown.txt", "first\nsecond\n");
      await statusBar.waitForEol("lf");

      await clickTestId("status-eol");
      const dialog = await byTestId("eol-picker-dialog");
      await dialog.waitForDisplayed({
        timeout: 10_000,
        timeoutMsg: "The EOL picker never opened.",
      });

      const trigger = await byTestId("eol-select-trigger");
      expect(await trigger.getText()).toContain("LF (Unix, macOS, Linux)");
      await trigger.click();

      const lfOption = await byTestId("eol-item-lf");
      await lfOption.waitForDisplayed({
        timeout: 10_000,
        timeoutMsg: "The LF option never appeared.",
      });
      expect(await lfOption.getAttribute("data-selected")).not.toBeNull();
      expect(await (await byTestId("eol-item-crlf")).getAttribute("data-selected")).toBeNull();
      // CR is a per-document value only; it must never be offered as a choice.
      expect(await existsTestId("eol-item-cr")).toBe(false);

      await clickTestId("eol-item-crlf");
      await statusBar.waitForEol("crlf");
      expect(await dialog.isDisplayed()).toBe(true);
      expect(await trigger.getText()).toContain("CRLF (Windows)");

      await pressEscape();
      await dialog.waitForDisplayed({
        reverse: true,
        timeout: 10_000,
        timeoutMsg: "The EOL picker never closed.",
      });
    },
  );

  scenario(
    "format.eol.hidden-in-csv",
    "hides the EOL control in CSV table mode without dirtying the file",
    async () => {
      const original = "id,name\r\n1,Alice\r\n2,Bob\r\n";
      const filePath = await openText("crlf-table.csv", original);
      await statusBar.waitForEol("crlf");

      // Table mode owns the surface and deliberately hides text-mode metadata.
      await csv.enter();
      expect(await existsTestId("status-eol")).toBe(false);
      await csv.exit();
      await statusBar.waitForEol("crlf");

      // The round trip must not rewrite the file: the CSV session serializes
      // canonical LF, and conversion happens only at the write boundary.
      expectFileBytes(filePath, original);
    },
  );

  scenario(
    "format.eol.autosave-generation",
    "keeps autosave generations monotonic after an EOL change and CSV round-trip",
    async () => {
      const filePath = provisionSlate("eol-generation.csv", "id,name\n1,Alice\n");
      await openPath(filePath);
      await statusBar.waitForEol("lf");

      await statusBar.selectEol("crlf");
      await waitForFile(filePath, (content) => content === "id,name\r\n1,Alice\r\n", {
        message: "The EOL change never autosaved.",
      });

      await csv.enter();
      expect(await existsTestId("status-eol")).toBe(false);
      await csv.exit();
      await statusBar.waitForEol("crlf");

      await editor.replaceText("id,name\n1,Alice\n2,Bob");

      // This edit must advance past the generation the CSV EOL autosave
      // completed. Reusing that generation makes Rust consider the edit clean,
      // and the write below never lands.
      await waitForFile(filePath, (content) => content === "id,name\r\n1,Alice\r\n2,Bob", {
        message: "The post-round-trip edit never autosaved; a generation was reused.",
      });
    },
  );

  scenario(
    "format.eol.save-cycles",
    "tracks dirty and save-enabled state across edits either side of a conversion",
    async () => {
      const filePath = await openText("eol-save-lifecycle.txt", "alpha\r\n");
      const beforeConversion = "alpha\nsaved before conversion\n";
      const afterConversion = `${beforeConversion}saved after conversion\n`;

      await statusBar.waitForEol("crlf");
      await titleBar.waitForDirty(false);
      await titleBar.waitForSaveEnabled(false);

      // Edit and save while still CRLF.
      await editor.replaceText(beforeConversion);
      await titleBar.waitForDirty(true);
      await titleBar.waitForSaveEnabled(true);
      await editor.save();
      await waitForFile(filePath, (c) => c === "alpha\r\nsaved before conversion\r\n", {
        message: "The pre-conversion save never landed.",
      });
      expectFileBytes(filePath, "alpha\r\nsaved before conversion\r\n");
      await titleBar.waitForDirty(false);
      await titleBar.waitForSaveEnabled(false);

      // Convert with no text change: still a savable difference.
      await statusBar.selectEol("lf");
      await titleBar.waitForDirty(true);
      await titleBar.waitForSaveEnabled(true);
      await editor.save();
      await waitForFile(filePath, (c) => c === beforeConversion, {
        message: "The conversion-only save never landed.",
      });
      expectFileBytes(filePath, beforeConversion);
      await titleBar.waitForDirty(false);

      // Edit again after the conversion; the new EOL must stick.
      await editor.replaceText(afterConversion);
      await titleBar.waitForDirty(true);
      await editor.save();
      await waitForFile(filePath, (c) => c === afterConversion, {
        message: "The post-conversion save never landed.",
      });
      expectFileBytes(filePath, afterConversion);
      await titleBar.waitForDirty(false);
      await titleBar.waitForSaveEnabled(false);
    },
  );

  scenario(
    "format.eol.clipboard-conversion",
    "copies multiline CRLF content without treating converted byte growth as failure",
    async () => {
      await openText("crlf-copy.txt", "north\r\nsouth\r\n");
      await statusBar.waitForEol("crlf");

      await clickTestId("action-copy");
      await waitForAttribute("action-copy", "data-copy-success", "true", {
        message: "The converted clipboard copy was never reported as successful.",
      });

      // Read the native clipboard directly so the assertion proves that the
      // document's CRLF format survives the copy boundary byte-for-byte.
      await waitForClipboardText("north\r\nsouth\r\n");
    },
  );

  scenario(
    "format.eol.no-trailing-newline",
    "preserves the absence of a trailing newline",
    async () => {
      const filePath = await openText("no-trailing.txt", "solo\r\nlast");
      await statusBar.waitForEol("crlf");

      await editor.replaceText("solo\nlast!");
      await editor.save();

      await waitForFile(filePath, (content) => content.includes("last!"), {
        message: "The edited file never saved.",
      });
      expectFileBytes(filePath, "solo\r\nlast!");
      expect(fs.readFileSync(filePath).at(-1)).not.toBe(0x0a);
    },
  );
});
