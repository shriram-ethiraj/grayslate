import fs from "node:fs";
import path from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { notesRoot, rustFixture } from "../helpers/sandbox.js";

async function typeText(text: string): Promise<void> {
  // `browser.keys(string)` sends all key-down events before the key-up events.
  // WebKit can treat adjacent identical key-downs as one auto-repeat (for
  // example, the two `s` characters in `process`), which makes the saved bytes
  // differ from what a user typed. Build one native key action with a complete
  // down/up pair for every character instead.
  const enterKey = "\uE007";
  const action = browser.action("key");

  for (const character of text) {
    const key = character === "\n" ? enterKey : character;
    action.down(key).pause(25).up(key).pause(25);
  }

  await action.perform();
}

describe("real Tauri editor flow", () => {
  it("detects Rust after typing, autosaves it, and gives it a Rust name", async () => {
    const editor = await $("[data-testid='editor'] .cm-content");
    await editor.waitForDisplayed();
    await editor.click();

    await typeText(rustFixture);

    await browser.waitUntil(
      async () => (await editor.getText()).includes("pub struct Config"),
      {
        timeout: 10_000,
        interval: 250,
        timeoutMsg: "The Rust fixture was not entered into CodeMirror.",
      },
    );

    const languageMode = await $("[data-testid='language-mode']");
    await languageMode.waitForDisplayed();
    await browser.waitUntil(
      async () =>
        (await languageMode.getAttribute("data-detected-language")) === "rust",
      {
        timeout: 10_000,
        interval: 250,
        timeoutMsg: "Rust content detection did not complete after typing stopped.",
      },
    );

    const expectedPath = path.join(notesRoot, "config.rs");
    await browser.waitUntil(
      async () => {
        try {
          return fs.readFileSync(expectedPath, "utf8") === rustFixture;
        } catch {
          return false;
        }
      },
      {
        timeout: 20_000,
        interval: 250,
        timeoutMsg:
          "The real Rust autosave did not create config.rs with the typed content.",
      },
    );

    await browser.waitUntil(
      async () =>
        (await languageMode.getAttribute("data-language-mode")) === "rust",
      {
        timeout: 5_000,
        interval: 250,
        timeoutMsg: "The saved Rust filename did not update the language mode.",
      },
    );

    expect(fs.existsSync(expectedPath)).toBe(true);
    expect(fs.readFileSync(expectedPath, "utf8")).toBe(rustFixture);
    expect(await languageMode.getAttribute("data-language-mode")).toBe("rust");
    expect(await languageMode.getText()).toContain("Rust");
  });
});
