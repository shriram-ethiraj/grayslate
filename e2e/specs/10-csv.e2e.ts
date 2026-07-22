import fs from "node:fs";
import path from "node:path";
import { $, browser, expect } from "@wdio/globals";
import {
  ALT,
  ARROW_DOWN,
  MOD,
  SHIFT,
  clickTestId,
  csvCell,
  enterCsvTable,
  exitCsvTable,
  focusEditor,
  openAuthorizedPath,
  openExternalFixture,
  pressMod,
  readEditorText,
  replaceEditorText,
  waitForEditorText,
  waitForFile,
  writeLargeCsv,
} from "../helpers/app.js";
import { notesRoot, sandboxRoot } from "../helpers/sandbox.js";

async function waitForCsvInfo(fragment: string): Promise<void> {
  const info = await $("[data-testid='status-csv-info']");
  await browser.waitUntil(async () => (await info.getText()).includes(fragment), {
    timeout: 15_000,
    interval: 200,
    timeoutMsg: `CSV status never contained '${fragment}'.`,
  });
}

async function editCsvCell(row: number, col: number, value: string): Promise<void> {
  await browser.execute((targetRow, targetCol) => {
    const cell = document.querySelector<HTMLElement>(
      `[data-row='${targetRow}'][data-col='${targetCol}']`,
    );
    if (!cell) throw new Error(`CSV cell ${targetRow},${targetCol} is missing.`);
    const rect = cell.getBoundingClientRect();
    cell.dispatchEvent(new MouseEvent("dblclick", {
      bubbles: true,
      clientX: rect.left + rect.width / 2,
      clientY: rect.top + rect.height / 2,
    }));
  }, row, col);
  const input = await $(".csv-edit-input");
  await input.waitForDisplayed();
  await browser.executeAsync((nextValue, done) => {
    const activeInput = document.querySelector<HTMLInputElement>(".csv-edit-input");
    if (!activeInput) {
      done("CSV edit input disappeared before the value could be applied.");
      return;
    }
    activeInput.value = nextValue;
    activeInput.dispatchEvent(new Event("input", { bubbles: true }));
    requestAnimationFrame(() => {
      activeInput.blur();
      done(null);
    });
  }, value);
}

async function csvCellText(row: number, col: number): Promise<string> {
  return browser.execute((targetRow, targetCol) =>
    document.querySelector<HTMLElement>(
      `[data-row='${targetRow}'][data-col='${targetCol}'] .csv-cell-content`,
    )?.textContent ?? "",
  row, col);
}

async function expectTooltip(testId: string, expectedText: string): Promise<void> {
  const trigger = await $(`[data-testid='${testId}']`);
  await trigger.moveTo();
  await browser.execute((id) => {
    const element = document.querySelector<HTMLElement>(`[data-testid='${id}']`);
    if (!element) throw new Error(`Tooltip trigger ${id} is missing.`);
    element.dispatchEvent(new PointerEvent("pointerenter", {
      bubbles: true,
      pointerType: "mouse",
    }));
  }, testId);
  await browser.waitUntil(async () => browser.execute((text) =>
    Array.from(document.querySelectorAll<HTMLElement>("[role='tooltip']")).some((tooltip) =>
      tooltip.getClientRects().length > 0 && tooltip.textContent?.trim() === text,
    ), expectedText), {
    timeoutMsg: `Tooltip for ${testId} did not show '${expectedText}'.`,
  });
}

describe("Act 10 — CSV table lifecycle", () => {
  let csvPath = "";

  it("enters table view and reports rows, columns, and delimiter", async () => {
    csvPath = await openExternalFixture("sample.csv");
    await enterCsvTable();
    await waitForCsvInfo("3 rows");
    const info = await $("[data-testid='status-csv-info']");
    expect(await info.getText()).toContain("3 cols");
    expect((await info.getText()).toLowerCase()).toContain("comma");
    const tableFamily = await browser.execute(() => {
      const table = document.querySelector<HTMLElement>("[data-testid='csv-table'] .csv-table");
      if (!table) throw new Error("CSV typography element is missing.");
      return getComputedStyle(table).fontFamily;
    });
    expect(tableFamily).toContain("Commit Mono");
  });

  it("explains unavailable toolbar actions in table mode", async () => {
    const transformations = await $("[data-testid='action-transformations']");
    expect(await transformations.getAttribute("aria-disabled")).toBe("true");
    await expectTooltip("action-transformations", "Not available in CSV table mode");

    const copy = await $("[data-testid='action-copy']");
    expect(await copy.getAttribute("aria-disabled")).toBe("true");
    await expectTooltip("action-copy", "Not available in CSV table mode");

    await transformations.click();
    await expect(await $("[data-testid='transformations-palette']")).not.toBeDisplayed();

    const save = await $("[data-testid='action-save']");
    expect(await save.getAttribute("aria-disabled")).toBe("true");
    await expectTooltip("action-save", "No changes to save");
  });

  it("shows an external cell edit immediately and saves without leaving table mode", async () => {
    await editCsvCell(0, 1, "Alice E2E");
    expect(await csvCellText(0, 1)).toBe("Alice E2E");

    const save = await $("[data-testid='action-save']");
    await browser.waitUntil(async () => (await save.getAttribute("aria-disabled")) === "false", {
      timeoutMsg: "Save did not become enabled after the table edit.",
    });
    await (await $("[data-testid='title-dirty-indicator']")).waitForDisplayed();

    await save.click();
    await waitForFile(csvPath, (content) => content.includes("Alice E2E"));
    await browser.waitUntil(async () => (await save.getAttribute("aria-disabled")) === "true", {
      timeoutMsg: "Save did not become disabled after persisting the table edit.",
    });
    await (await $("[data-testid='title-dirty-indicator']")).waitForExist({ reverse: true });
    await expect(await $("[data-testid='csv-table']")).toBeDisplayed();
  });

  it("navigates, edits, clears, and undo-redoes cells", async () => {
    const first = await csvCell(0, 0);
    await first.click();
    await browser.keys(ARROW_DOWN);
    expect(await (await csvCell(1, 0)).getAttribute("aria-selected")).toBe("true");

    await editCsvCell(0, 1, "");
    await browser.waitUntil(async () => (await csvCellText(0, 1)) === "");
    const save = await $("[data-testid='action-save']");
    await browser.waitUntil(async () => (await save.getAttribute("aria-disabled")) === "false");
    await (await csvCell(0, 2)).click();
    await pressMod("z");
    await browser.waitUntil(async () => (await csvCellText(0, 1)) === "Alice E2E");
    await browser.waitUntil(async () => (await save.getAttribute("aria-disabled")) === "true", {
      timeoutMsg: "Undoing to the saved table state did not clear dirty state.",
    });
    await (await csvCell(0, 2)).click();
    await browser.keys([MOD, SHIFT, "z"]);
    await browser.waitUntil(async () => (await csvCellText(0, 1)) === "");
    await browser.waitUntil(async () => (await save.getAttribute("aria-disabled")) === "false");
    await (await csvCell(0, 2)).click();
    await pressMod("z");
    await browser.waitUntil(async () => (await save.getAttribute("aria-disabled")) === "true");
  });

  it("inserts and moves a row, then returns all edits to text mode", async () => {
    const row = await csvCell(1, -1);
    await row.click();
    await browser.keys([MOD, ALT, ARROW_DOWN]);
    await waitForCsvInfo("4 rows");
    await browser.keys([ALT, ARROW_DOWN]);

    await exitCsvTable();
    await waitForEditorText((text) => text.includes("Alice E2E"));
    expect(await readEditorText()).toContain("Alice E2E");
  });

  it("saves table edits and validates the bytes on disk", async () => {
    await focusEditor();
    await pressMod("s");
    await waitForFile(csvPath, (content) => content.includes("Alice E2E"));
    expect(fs.readFileSync(csvPath, "utf8")).toContain("Alice E2E");
  });

  it("autosaves slate table edits while remaining in table mode", async () => {
    const slatePath = path.join(notesRoot, "table-autosave.csv");
    fs.writeFileSync(slatePath, "id,name\n1,Original\n", "utf8");
    await openAuthorizedPath(slatePath);
    await enterCsvTable();

    await (await $("[data-testid='action-save']")).waitForExist({ reverse: true });
    await editCsvCell(0, 1, "Slate E2E");
    expect(await csvCellText(0, 1)).toBe("Slate E2E");

    await waitForFile(slatePath, (content) => content.includes("Slate E2E"));
    expect(fs.readFileSync(slatePath, "utf8")).toContain("Slate E2E");
    await expect(await $("[data-testid='csv-table']")).toBeDisplayed();

    await exitCsvTable();
    await waitForEditorText((text) => text.includes("Slate E2E"));
    await replaceEditorText("id,name\n1,Slate text edit");
    await waitForFile(slatePath, (content) => content.includes("Slate text edit"));

    await enterCsvTable();
    expect(await csvCellText(0, 1)).toBe("Slate text edit");
    await expect(await $("[data-testid='csv-table']")).toBeDisplayed();
  });

  it("preserves external dirty state through text and table mode switches", async () => {
    const modeSwitchPath = path.join(sandboxRoot, "mode-switch.csv");
    fs.writeFileSync(modeSwitchPath, "id,name\n1,Original\n", "utf8");
    await openAuthorizedPath(modeSwitchPath);
    await waitForEditorText((text) => text.includes("Original"));

    const save = await $("[data-testid='action-save']");
    await replaceEditorText("id,name\n1,Text edit");
    await browser.waitUntil(async () => (await save.getAttribute("aria-disabled")) === "false");
    await (await $("[data-testid='title-dirty-indicator']")).waitForDisplayed();

    await enterCsvTable();
    expect(await csvCellText(0, 1)).toBe("Text edit");
    expect(await save.getAttribute("aria-disabled")).toBe("false");

    await editCsvCell(0, 1, "Table edit");
    expect(await csvCellText(0, 1)).toBe("Table edit");
    await exitCsvTable();
    await waitForEditorText((text) => text.includes("Table edit"));
    expect(await save.getAttribute("aria-disabled")).toBe("false");

    await save.click();
    await waitForFile(modeSwitchPath, (content) => content.includes("Table edit"));
    await browser.waitUntil(async () => (await save.getAttribute("aria-disabled")) === "true");
    await (await $("[data-testid='title-dirty-indicator']")).waitForExist({ reverse: true });

    await enterCsvTable();
    expect(await csvCellText(0, 1)).toBe("Table edit");
    expect(await save.getAttribute("aria-disabled")).toBe("true");
  });

  it("keeps large CSV rendering bounded and returns to text in one undo step", async () => {
    const largePath = path.join(sandboxRoot, "large.csv");
    writeLargeCsv(largePath, 100_001);
    await openAuthorizedPath(largePath);
    await waitForEditorText((text) => text.startsWith("id,name,value"), 30_000);
    const originalStart = (await readEditorText()).slice(0, 80);

    await enterCsvTable();
    await waitForCsvInfo("100001 rows");
    const renderedRows = await browser.execute(() =>
      document.querySelectorAll("[data-testid='csv-table'] [data-row]").length,
    );
    expect(renderedRows).toBeLessThanOrEqual(200 * 4);

    await editCsvCell(0, 1, "large-edit");
    const save = await $("[data-testid='action-save']");
    await browser.waitUntil(async () => (await save.getAttribute("aria-disabled")) === "false", {
      timeoutMsg: "Large non-mirrored table edit did not enable Save.",
    });
    await (await $("[data-testid='title-dirty-indicator']")).waitForDisplayed();
    await exitCsvTable();
    await waitForEditorText((text) => text.includes("large-edit"), 30_000);
    expect(await save.getAttribute("aria-disabled")).toBe("false");

    await focusEditor();
    await pressMod("z");
    await waitForEditorText((text) => text.slice(0, 80) === originalStart, 30_000);
  });
});
