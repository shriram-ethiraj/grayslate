import fs from "node:fs";
import path from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  clickTestId,
  editorContent,
  ensureSidebarOpen,
  focusEditor,
  grantSavePath,
  invokeInApp,
  newSlate,
  openExternalFixture,
  pressMod,
  setFilterTab,
  sidebarCard,
  typeText,
  waitForDetectedLanguage,
  waitForFile,
} from "../helpers/app.js";
import { externalRoot } from "../helpers/app.js";
import { notesRoot } from "../helpers/sandbox.js";

// Opening a file that lives outside the notes root exercises the real
// `read_file_content` + authorization path (via the `e2e_open_path` shim) and
// classifies it as a tracked *local* file — the "Local" sidebar tab, never
// "Slates". A native open dialog cannot be driven by WebDriver, so the shim
// substitutes the fixture path for the picker while keeping the grant path.
describe("Act 2 — external / local files", () => {
  let externalPath = "";

  it("opens an external file under All and Local, not Slates", async () => {
    externalPath = await openExternalFixture("sample.py");

    const editor = await editorContent();
    await browser.waitUntil(
      async () => (await editor.getText()).includes("def greet"),
      {
        timeout: 10_000,
        interval: 250,
        timeoutMsg: "The external Python fixture did not load into the editor.",
      },
    );
    await waitForDetectedLanguage("python");

    await ensureSidebarOpen();

    await setFilterTab("unified");
    await (await sidebarCard(externalPath)).waitForDisplayed();

    await setFilterTab("local");
    await (await sidebarCard(externalPath)).waitForDisplayed();

    await setFilterTab("slates");
    await (await sidebarCard(externalPath)).waitForExist({ reverse: true, timeout: 5_000 });

    await setFilterTab("unified");
  });

  it("edits and saves the external file; disk is updated and it stays in All + Local", async () => {
    await focusEditor();
    await typeText("# edited-by-e2e\n");
    await pressMod("s");

    await waitForFile(externalPath, (content) => content.includes("# edited-by-e2e"), 15_000);
    expect(fs.readFileSync(externalPath, "utf8")).toContain("# edited-by-e2e");

    await ensureSidebarOpen();
    await setFilterTab("local");
    await (await sidebarCard(externalPath)).waitForDisplayed();
    await setFilterTab("unified");
    await (await sidebarCard(externalPath)).waitForDisplayed();
  });

  it("writes a Save-As grant to a chosen path and opens the new authorized document", async () => {
    const target = path.join(externalRoot, "saved-as.py");
    const descriptor = await grantSavePath(target);
    expect(descriptor).not.toBeNull();
    if (!descriptor) throw new Error("Save-As did not return a document grant.");

    const content = "def saved_as():\n    return True\n";
    await invokeInApp("write_file_content", {
      documentId: descriptor.documentId,
      documentGeneration: descriptor.generation,
      content,
    });
    await waitForFile(target, (value) => value === content);
    expect(fs.readFileSync(target, "utf8")).toBe(content);

    await invokeInApp("e2e_open_path", { path: target });
    await browser.waitUntil(async () => (await editorContent()).getText().then((value) => value.includes("saved_as")));
  });

  it("guards unsaved local changes and supports cancel and discard", async () => {
    await focusEditor();
    await typeText("# unsaved guard\n");
    await newSlate();

    const dialog = await $("[data-testid='unsaved-changes-dialog']");
    await dialog.waitForDisplayed();
    await clickTestId("unsaved-cancel");
    await dialog.waitForDisplayed({ reverse: true });
    expect((await (await editorContent()).getText())).toContain("unsaved guard");

    await newSlate();
    await dialog.waitForDisplayed();
    await clickTestId("unsaved-discard");
    await dialog.waitForDisplayed({ reverse: true });
    const title = await $("[data-testid='title-file-name']");
    await browser.waitUntil(async () => (await title.getAttribute("title")) === "New Slate", {
      timeoutMsg: "Discarding changes did not finish opening a new slate.",
    });
  });

  it("coalesces repeated Save shortcuts with autosave into one new slate", async () => {
    const content = "save serialization regression content";
    await focusEditor();
    await typeText(content);

    await pressMod("s");
    await pressMod("s");
    await pressMod("s");

    await browser.waitUntil(
      () => fs.readdirSync(notesRoot).some((name) => {
        const candidate = path.join(notesRoot, name);
        return fs.statSync(candidate).isFile() && fs.readFileSync(candidate, "utf8") === content;
      }),
      {
        timeout: 15_000,
        interval: 200,
        timeoutMsg: "Repeated Save did not persist the new slate.",
      },
    );

    // Let a pending timer autosave settle, then verify it reused the manual
    // save instead of creating a content-identical suffixed filename.
    await browser.pause(2_500);
    const matchingFiles = fs.readdirSync(notesRoot).filter((name) => {
      const candidate = path.join(notesRoot, name);
      return fs.statSync(candidate).isFile() && fs.readFileSync(candidate, "utf8") === content;
    });
    expect(matchingFiles).toHaveLength(1);
  });
});
