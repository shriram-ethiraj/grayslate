import fs from "node:fs";
import path from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  clickTestId,
  enterCsvTable,
  exitCsvTable,
  focusEditor,
  openAuthorizedPath,
  waitForDirtyState,
  openExternalText,
  pressMod,
  replaceEditorText,
  selectEol,
  typeText,
  waitForEditorText,
  waitForEol,
  waitForFile,
} from "../helpers/app.js";
import { waitForClipboardText } from "../helpers/clipboard.js";
import { notesRoot } from "../helpers/sandbox.js";

// The editor holds every document canonically LF-terminated (CodeMirror
// enforces this), so the line ending is per-document metadata applied only at
// the disk-write boundary. These specs assert raw bytes rather than decoded
// strings, because a dropped `\r` is invisible in a string comparison.
describe("Act 12 — line endings", () => {
  it("detects CRLF, opens clean, and leaves the file byte-identical", async () => {
    const original = "alpha\r\nbeta\r\ngamma\r\n";
    const filePath = await openExternalText("crlf-clean.txt", original);

    await waitForEol("crlf");
    // The regression this feature fixes: comparing the editor's normalized text
    // against raw CRLF bytes used to mark every Windows file dirty on open, then
    // silently rewrite it as LF. Clean-on-open means the bytes stay untouched.
    await waitForDirtyState(false);

    expect(fs.readFileSync(filePath)).toEqual(Buffer.from(original, "utf8"));
  });

  it("keeps CRLF when the file is edited and saved", async () => {
    const filePath = await openExternalText("crlf-edit.txt", "one\r\ntwo\r\n");
    await waitForEol("crlf");

    await focusEditor();
    await pressMod("a");
    await typeText("one\ntwo\nthree\n");
    await waitForEditorText((text) => text.includes("three"));
    await pressMod("s");

    await waitForFile(filePath, (content) => content.includes("three"), 15_000);
    expect(fs.readFileSync(filePath)).toEqual(
      Buffer.from("one\r\ntwo\r\nthree\r\n", "utf8"),
    );
  });

  it("opens legacy CR cleanly and modernizes it to LF on the next write", async () => {
    const original = "red\rgreen\r";
    const filePath = await openExternalText("cr-legacy.txt", original);
    await waitForEol("lf");
    await waitForDirtyState(false);
    expect(fs.readFileSync(filePath)).toEqual(Buffer.from(original, "utf8"));

    await focusEditor();
    await pressMod("a");
    await typeText("red\ngreen\nblue\n");
    await waitForEditorText((text) => text.includes("blue"));
    await pressMod("s");

    await waitForFile(filePath, (content) => content.includes("blue"), 15_000);
    expect(fs.readFileSync(filePath)).toEqual(
      Buffer.from("red\ngreen\nblue\n", "utf8"),
    );
  });

  it("uses the dominant style for a mixed file and normalizes on save", async () => {
    // 3 CRLF vs 1 lone LF — CRLF dominates and is what the status bar reports.
    const filePath = await openExternalText(
      "mixed.txt",
      "a\r\nb\r\nc\r\nd\ne",
    );
    await waitForEol("crlf");

    // A genuine edit (append a line) is needed to dirty the document — retyping
    // the normalized original would be a no-op and never trigger a save.
    await focusEditor();
    await pressMod("a");
    await typeText("a\nb\nc\nd\ne\nf");
    await waitForEditorText((text) => text.endsWith("f"));
    await pressMod("s");

    // Every break, including the previously lone LF, is now CRLF.
    await waitForFile(filePath, (content) => content.includes("f"), 15_000);
    expect(fs.readFileSync(filePath)).toEqual(
      Buffer.from("a\r\nb\r\nc\r\nd\r\ne\r\nf", "utf8"),
    );
  });

  it("marks the document dirty on an EOL switch and converts on save", async () => {
    const filePath = await openExternalText("switch.txt", "x\r\ny\r\n");
    await waitForEol("crlf");
    await waitForDirtyState(false);

    // Changing the line ending is a real, savable change even though the
    // canonical text is untouched.
    await selectEol("lf");
    await waitForDirtyState(true);

    await pressMod("s");
    await waitForFile(filePath, (content) => !content.includes("\r"), 15_000);
    expect(fs.readFileSync(filePath)).toEqual(Buffer.from("x\ny\n", "utf8"));
    await waitForDirtyState(false);
  });

  it("clears dirty state when the EOL is switched back before saving", async () => {
    await openExternalText("revert.txt", "p\r\nq\r\n");
    await waitForEol("crlf");
    await waitForDirtyState(false);

    await selectEol("lf");
    await waitForDirtyState(true);

    await selectEol("crlf");
    await waitForDirtyState(false);
  });

  it("shows an EOL dropdown and stays open after applying a choice", async () => {
    await openExternalText("picker-dropdown.txt", "first\nsecond\n");
    await waitForEol("lf");

    await clickTestId("status-eol");
    const dialog = await $("[data-testid='eol-picker-dialog']");
    await dialog.waitForDisplayed();

    const trigger = await $("[data-testid='eol-select-trigger']");
    expect(await trigger.getText()).toContain("LF (Unix, macOS, Linux)");
    await trigger.click();

    const lfOption = await $("[data-testid='eol-item-lf']");
    const crlfOption = await $("[data-testid='eol-item-crlf']");
    await lfOption.waitForDisplayed();
    expect(await lfOption.getAttribute("data-selected")).not.toBeNull();
    expect(await crlfOption.getAttribute("data-selected")).toBeNull();
    expect(await $("[data-testid='eol-item-cr']").isExisting()).toBe(false);

    await crlfOption.click();
    await waitForEol("crlf");
    expect(await dialog.isDisplayed()).toBe(true);
    expect(await trigger.getText()).toContain("CRLF (Windows)");

    await browser.keys("Escape");
    await dialog.waitForDisplayed({ reverse: true });
  });

  it("hides the EOL control in CSV table mode without dirtying the file", async () => {
    const original = "id,name\r\n1,Alice\r\n2,Bob\r\n";
    const filePath = await openExternalText("crlf-table.csv", original);
    await waitForEol("crlf");

    // Table mode owns the surface and intentionally hides text-mode metadata
    // controls. Leaving table mode restores the document's detected EOL.
    await enterCsvTable();
    expect(await $("[data-testid='status-eol']").isExisting()).toBe(false);
    await exitCsvTable();
    await waitForEol("crlf");

    // Round-tripping through the CSV session must not rewrite the file: the
    // session serializes canonical LF and conversion happens only on write.
    expect(fs.readFileSync(filePath)).toEqual(Buffer.from(original, "utf8"));
  });

  it("keeps autosave generations monotonic after an EOL change and CSV round-trip", async () => {
    const filePath = path.join(notesRoot, "eol-generation.csv");
    fs.writeFileSync(filePath, "id,name\n1,Alice\n", "utf8");
    await openAuthorizedPath(filePath);
    await waitForEol("lf");

    await selectEol("crlf");
    await waitForFile(
      filePath,
      (content) => content === "id,name\r\n1,Alice\r\n",
      20_000,
    );

    await enterCsvTable();
    expect(await $("[data-testid='status-eol']").isExisting()).toBe(false);
    await exitCsvTable();
    await waitForEol("crlf");
    await replaceEditorText("id,name\n1,Alice\n2,Bob");

    // This edit must advance beyond the generation completed by the CSV EOL
    // autosave. Reusing that generation makes Rust consider the edit clean.
    await waitForFile(
      filePath,
      (content) => content === "id,name\r\n1,Alice\r\n2,Bob",
      20_000,
    );
  });

  it("copies multiline CRLF content without treating converted byte growth as failure", async () => {
    const canonical = "north\nsouth\n";
    await openExternalText("crlf-copy.txt", "north\r\nsouth\r\n");
    await waitForEol("crlf");

    await clickTestId("action-copy");
    const copyAction = await $("[data-testid='action-copy']");
    await browser.waitUntil(
      async () => (await copyAction.getAttribute("data-copy-success")) === "true",
      {
        timeout: 10_000,
        interval: 100,
        timeoutMsg: "Converted clipboard copy was not reported as successful.",
      },
    );
    // Textarea values normalize platform clipboard line endings to LF.
    await waitForClipboardText(canonical);
  });

  it("preserves the absence of a trailing newline", async () => {
    const filePath = await openExternalText("no-trailing.txt", "solo\r\nlast");
    await waitForEol("crlf");

    await focusEditor();
    await pressMod("a");
    await typeText("solo\nlast!");
    await waitForEditorText((text) => text.endsWith("last!"));
    await pressMod("s");

    await waitForFile(filePath, (content) => content.includes("last!"), 15_000);
    expect(fs.readFileSync(filePath)).toEqual(
      Buffer.from("solo\r\nlast!", "utf8"),
    );
  });
});
