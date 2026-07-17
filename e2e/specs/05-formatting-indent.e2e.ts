import { $, browser, expect } from "@wdio/globals";
import {
  clickTestId,
  focusEditor,
  openExternalFixture,
  pressMod,
  readEditorText,
  runTransform,
  waitForEditorText,
} from "../helpers/app.js";

async function chooseVisibleOption(label: string): Promise<void> {
  const option = await $(`//*[ @role='option' and normalize-space(.)='${label}' ]`);
  await option.waitForDisplayed();
  await option.click();
}

describe("Act 5 — formatting and indentation", () => {
  it("changes indentation mode and size from the status picker", async () => {
    await openExternalFixture("messy.json");
    await clickTestId("status-indent");
    await (await $("[data-testid='indent-picker']")).waitForDisplayed();

    await clickTestId("indent-mode-trigger");
    await chooseVisibleOption("Spaces");
    await clickTestId("indent-size-trigger");
    await chooseVisibleOption("4");
    expect(await (await $("[data-testid='status-indent']")).getText()).toContain("Spaces: 4");

    await clickTestId("indent-mode-trigger");
    await chooseVisibleOption("Tab");
    expect(await (await $("[data-testid='status-indent']")).getText()).toContain("Tab");
    await browser.keys("Escape");
  });

  it("formats JSON using one undoable editor transaction", async () => {
    const original = await readEditorText();
    await runTransform("json.format");
    await waitForEditorText((text) => text.includes("\n") && text.includes('"nested"'));
    const formatted = await readEditorText();
    expect(formatted).not.toBe(original);

    await focusEditor();
    await pressMod("z");
    await waitForEditorText((text) => text === original);
  });

  it("formats SQL and fully reverts with a single undo", async () => {
    await openExternalFixture("unformatted.sql");
    await waitForEditorText((text) => text.trimEnd() === "select a,b from t where a=1 and b=2 order by a");
    const original = await readEditorText();
    await runTransform("sql.format");
    await waitForEditorText((text) => text !== original && text.toUpperCase().includes("SELECT"));

    await focusEditor();
    await pressMod("z");
    await waitForEditorText((text) => text === original);
  });
});
