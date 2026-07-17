import fs from "node:fs";
import path from "node:path";
import { browser, expect } from "@wdio/globals";
import { notesRoot, rustFixture } from "../helpers/sandbox.js";
import {
  editorContent,
  ensureSidebarOpen,
  focusEditor,
  newSlate,
  openSidebarCard,
  setFilterTab,
  sidebarCard,
  typeText,
  waitForDetectedLanguage,
  waitForFile,
  waitForLanguageMode,
} from "../helpers/app.js";

// The Rust fixture contains `pub struct Config`, so the naming pipeline saves
// it as `config.rs`. SQL is single-line to avoid any auto-indent byte drift,
// and its stem is content-derived, so the spec discovers the actual `.sql` file
// instead of hard-coding a name.
const rustPath = path.join(notesRoot, "config.rs");
const sqlContent = "SELECT id, name FROM users WHERE active = 1;";

describe("Act 1 — first run and the slate lifecycle", () => {
  it("detects Rust after typing, autosaves config.rs, and names it Rust", async () => {
    await focusEditor();
    await typeText(rustFixture);

    const editor = await editorContent();
    await browser.waitUntil(
      async () => (await editor.getText()).includes("pub struct Config"),
      {
        timeout: 10_000,
        interval: 250,
        timeoutMsg: "The Rust fixture was not entered into CodeMirror.",
      },
    );

    await waitForDetectedLanguage("rust");
    await waitForFile(rustPath, (content) => content === rustFixture, 20_000);
    await waitForLanguageMode("rust");

    expect(fs.existsSync(rustPath)).toBe(true);
    expect(fs.readFileSync(rustPath, "utf8")).toBe(rustFixture);
  });

  it("shows the autosaved Rust slate in the sidebar under All and Slates, not Local", async () => {
    await ensureSidebarOpen();

    await setFilterTab("unified");
    await (await sidebarCard(rustPath)).waitForDisplayed();

    await setFilterTab("slates");
    await (await sidebarCard(rustPath)).waitForDisplayed();

    await setFilterTab("local");
    await (await sidebarCard(rustPath)).waitForExist({ reverse: true, timeout: 5_000 });

    await setFilterTab("unified");
  });

  it("new slate: types SQL, detects and autosaves it, and shows both slates", async () => {
    await newSlate();
    await focusEditor();
    await typeText(sqlContent);

    const editor = await editorContent();
    await browser.waitUntil(
      async () => (await editor.getText()).includes("SELECT id"),
      {
        timeout: 10_000,
        interval: 250,
        timeoutMsg: "The SQL content was not entered into CodeMirror.",
      },
    );

    await waitForDetectedLanguage("sql");

    // Discover the content-named `.sql` file the naming pipeline produced.
    let sqlPath = "";
    await browser.waitUntil(
      () => {
        const match = fs.readdirSync(notesRoot).find((name) => name.endsWith(".sql"));
        if (!match) return false;
        sqlPath = path.join(notesRoot, match);
        return fs.readFileSync(sqlPath, "utf8") === sqlContent;
      },
      {
        timeout: 20_000,
        interval: 250,
        timeoutMsg: "SQL autosave did not create a .sql file with the typed content.",
      },
    );

    await waitForLanguageMode("sql");

    await ensureSidebarOpen();
    await setFilterTab("unified");
    await (await sidebarCard(rustPath)).waitForDisplayed();
    await (await sidebarCard(sqlPath)).waitForDisplayed();
  });

  it("reopens the first file from the sidebar with content and language restored", async () => {
    await ensureSidebarOpen();
    await setFilterTab("unified");
    await openSidebarCard(rustPath);

    const editor = await editorContent();
    await browser.waitUntil(
      async () => (await editor.getText()).includes("pub struct Config"),
      {
        timeout: 10_000,
        interval: 250,
        timeoutMsg: "The reopened Rust file content did not load.",
      },
    );

    await waitForLanguageMode("rust");
    expect(fs.readFileSync(rustPath, "utf8")).toBe(rustFixture);
  });
});
