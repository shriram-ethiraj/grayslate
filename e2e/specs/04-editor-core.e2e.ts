import { $, browser, expect } from "@wdio/globals";
import {
  MOD,
  SHIFT,
  clickTestId,
  focusEditor,
  openExternalText,
  pressMod,
  readEditorText,
  replaceEditorText,
  typeText,
  waitForEditorText,
} from "../helpers/app.js";
import { waitForClipboardText } from "../helpers/clipboard.js";

const FIND_TEXT = "alpha\nbeta alpha\nALPHA\nalphabet";

async function waitForMatchCount(count: number): Promise<void> {
  const counter = await $("[data-testid='find-match-count']");
  await browser.waitUntil(async () => {
    const text = await counter.getText();
    return text === `${count}+` || text.endsWith(`/${count}`);
  }, {
    timeout: 10_000,
    interval: 200,
    timeoutMsg: `Find match count never became ${count}.`,
  });
}

describe("Act 4 — core editor editing", () => {
  it("finds, filters, replaces all, and undoes the replacement as one transaction", async () => {
    await openExternalText("editor-core.txt", FIND_TEXT);
    await waitForEditorText((text) => text === FIND_TEXT);

    await focusEditor();
    await pressMod("h");
    const findInput = await $("[data-testid='find-input']");
    await findInput.waitForDisplayed();
    await findInput.setValue("alpha");
    await waitForMatchCount(4);

    await clickTestId("find-opt-word");
    await waitForMatchCount(3);
    await clickTestId("find-opt-case");
    await waitForMatchCount(2);

    const replaceInput = await $("[data-testid='replace-input']");
    await replaceInput.waitForDisplayed();
    await replaceInput.setValue("omega");
    await clickTestId("find-replace-all");
    await waitForEditorText((text) => text === "omega\nbeta omega\nALPHA\nalphabet");

    await browser.keys("Escape");
    await focusEditor();
    await pressMod("z");
    await waitForEditorText((text) => text === FIND_TEXT);
  });

  it("navigates to a line and rejects an out-of-range line", async () => {
    await focusEditor();
    await pressMod("g");
    const input = await $("[data-testid='go-to-line-input']");
    await input.waitForDisplayed();
    await input.setValue("3");
    await browser.keys("Enter");
    expect(await (await $("[data-testid='status-goto-line']")).getText()).toContain("Ln 3");

    await clickTestId("status-goto-line");
    await input.waitForDisplayed();
    await input.setValue("99");
    await browser.keys("Enter");
    expect(await input.getAttribute("aria-invalid")).toBe("true");
    await browser.keys("Escape");
  });

  it("toggles word wrap, changes font size, and round-trips undo/redo", async () => {
    await focusEditor();
    await clickTestId("menu-edit");
    const wrapItem = await $("[data-testid='menu-word-wrap']");
    const wrappedBefore = await wrapItem.getAttribute("aria-checked");
    await wrapItem.click();
    await clickTestId("menu-edit");
    await browser.waitUntil(async () =>
      (await $("[data-testid='menu-word-wrap']")).getAttribute("aria-checked")
        .then((value) => value !== wrappedBefore),
    );
    await browser.keys("Escape");

    const content = await $("[data-testid='editor'] .cm-content");
    const fontBefore = await content.getCSSProperty("font-size");
    await clickTestId("menu-view");
    await clickTestId("menu-increase-font");
    await browser.waitUntil(async () => (await content.getCSSProperty("font-size")).value !== fontBefore.value);
    await clickTestId("menu-view");
    await clickTestId("menu-reset-font");
    await browser.waitUntil(
      async () => (await content.getCSSProperty("font-size")).value === "14px",
    );

    await replaceEditorText("undo-base");
    await focusEditor();
    await typeText("-change");
    await waitForEditorText((text) => text === "undo-base-change");
    await pressMod("z");
    await waitForEditorText((text) => text === "undo-base");
    if (process.platform === "darwin") {
      await browser.keys([MOD, SHIFT, "z"]);
    } else {
      await pressMod("y");
    }
    await waitForEditorText((text) => text === "undo-base-change");
    expect(await readEditorText()).toBe("undo-base-change");
  });

  it("copies the complete text document through the native clipboard", async () => {
    const text = "first line\nsecond line with commas, quotes, and symbols: []{}";
    await replaceEditorText(text);
    await clickTestId("action-copy");
    await waitForClipboardText(text);
  });
});
