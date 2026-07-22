import { $, browser } from "@wdio/globals";
import { pressMod } from "./app.js";

async function pasteClipboardText(): Promise<string> {
  await browser.execute(() => {
    document.querySelector("[data-testid='clipboard-paste-target']")?.remove();
    const textarea = document.createElement("textarea");
    textarea.dataset.testid = "clipboard-paste-target";
    textarea.style.cssText =
      "position:fixed;left:8px;top:8px;width:240px;height:80px;z-index:2147483647";
    document.body.appendChild(textarea);
    textarea.focus();
  });

  const target = await $("[data-testid='clipboard-paste-target']");
  await target.waitForDisplayed();
  await target.click();
  await pressMod("v");
  const value = await target.getValue();
  await browser.execute(() => {
    document.querySelector("[data-testid='clipboard-paste-target']")?.remove();
  });
  return value;
}

/** Read the native clipboard through a trusted OS paste event. */
export async function waitForClipboardText(expected: string): Promise<void> {
  let actual = "";
  try {
    await browser.waitUntil(async () => {
      actual = await pasteClipboardText();
      return actual === expected;
    }, {
      timeout: 10_000,
      interval: 100,
      timeoutMsg: "Clipboard text did not match the expected content.",
    });
  } catch {
    throw new Error(
      `Clipboard text did not match. Expected ${JSON.stringify(expected)}, ` +
        `received ${JSON.stringify(actual)}.`,
    );
  }
}
