import fs from "node:fs";
import { browser, expect } from "@wdio/globals";
import {
  editorContent,
  ensureSidebarOpen,
  focusEditor,
  openExternalFixture,
  pressMod,
  setFilterTab,
  sidebarCard,
  typeText,
  waitForDetectedLanguage,
  waitForFile,
} from "../helpers/app.js";

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
});
