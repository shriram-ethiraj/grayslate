import { $, browser, expect } from "@wdio/globals";
import {
  ARROW_RIGHT,
  HOME,
  SHIFT,
  focusEditor,
  openExternalFixture,
  openExternalText,
  pressMod,
  readEditorText,
  replaceEditorText,
  runTransform,
  waitForEditorText,
  waitForLanguageMode,
} from "../helpers/app.js";

describe("Act 6 — transformations", () => {
  it("filters the palette and applies deterministic replace-text actions", async () => {
    await openExternalText("transforms.txt", "beta\nalpha");
    await focusEditor();
    await pressMod("k");
    const palette = await $("[data-testid='transformations-palette']");
    await palette.waitForDisplayed();
    const search = await $("input[placeholder='Search transformations...']");
    await search.setValue("uppercase");
    await expect(await $("[data-testid='transform-item-text.uppercase']")).toBeDisplayed();
    await browser.keys("Escape");

    await runTransform("text.uppercase");
    await waitForEditorText((text) => text === "BETA\nALPHA");
    await focusEditor();
    await pressMod("z");
    await waitForEditorText((text) => text === "beta\nalpha");

    await runTransform("text.reverse-lines");
    await waitForEditorText((text) => text === "alpha\nbeta");
  });

  it("applies a supported transform only to the selected range", async () => {
    await replaceEditorText("alpha beta");
    await focusEditor();
    await browser.keys(HOME);
    const selection = browser.action("key").down(SHIFT);
    for (let index = 0; index < 5; index += 1) {
      selection.down(ARROW_RIGHT).up(ARROW_RIGHT);
    }
    await selection.up(SHIFT).perform();
    await runTransform("text.uppercase", false);
    await waitForEditorText((text) => text === "ALPHA beta");
  });

  it("shows statistics without mutating the document", async () => {
    const before = await readEditorText();
    await runTransform("stats.count-words");
    const toast = await $("[data-sonner-toast]");
    await toast.waitForDisplayed();
    expect((await toast.getText()).toLowerCase()).toContain("word");
    expect(await readEditorText()).toBe(before);
  });

  it("switches the active language after a format-converting action", async () => {
    await openExternalFixture("sample.csv");
    await waitForLanguageMode("csv");
    await runTransform("csv.to-json");
    await waitForEditorText((text) => text.trimStart().startsWith("["));
    await waitForLanguageMode("json");
  });

  it("assembles a chunked large result and keeps it as one undo step", async () => {
    const source = `${"chunked transport line\n".repeat(200_000)}done`;
    await openExternalText("big.txt", source);
    await runTransform("text.uppercase");
    await waitForEditorText((text) => text.startsWith("CHUNKED TRANSPORT LINE"), 30_000);
    expect(Number(await (await $("[data-testid='status-length']")).getAttribute("data-doc-length")))
      .toBe(source.length);

    await focusEditor();
    await pressMod("z");
    await waitForEditorText((text) => text.startsWith("chunked transport line"), 30_000);
  });
});
