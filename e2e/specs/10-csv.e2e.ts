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
  invokeInApp,
  openExternalFixture,
  pressMod,
  readEditorText,
  waitForEditorText,
  waitForFile,
  writeLargeCsv,
} from "../helpers/app.js";
import { sandboxRoot } from "../helpers/sandbox.js";

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

describe("Act 10 — CSV table lifecycle", () => {
  let csvPath = "";

  it("enters table view and reports rows, columns, and delimiter", async () => {
    csvPath = await openExternalFixture("sample.csv");
    await enterCsvTable();
    await waitForCsvInfo("3 rows");
    const info = await $("[data-testid='status-csv-info']");
    expect(await info.getText()).toContain("3 cols");
    expect((await info.getText()).toLowerCase()).toContain("comma");
  });

  it("navigates, edits, clears, and undo-redoes cells", async () => {
    const first = await csvCell(0, 0);
    await first.click();
    await browser.keys(ARROW_DOWN);
    expect(await (await csvCell(1, 0)).getAttribute("aria-selected")).toBe("true");

    await editCsvCell(0, 1, "Alice E2E");
    await browser.waitUntil(async () => (await csvCellText(0, 1)) === "Alice E2E");

    await editCsvCell(0, 1, "");
    await browser.waitUntil(async () => (await csvCellText(0, 1)) === "");
    await (await csvCell(0, 2)).click();
    await pressMod("z");
    await browser.waitUntil(async () => (await csvCellText(0, 1)) === "Alice E2E");
    await (await csvCell(0, 2)).click();
    await browser.keys([MOD, SHIFT, "z"]);
    await browser.waitUntil(async () => (await csvCellText(0, 1)) === "");
    await (await csvCell(0, 2)).click();
    await pressMod("z");
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

  it("keeps large CSV rendering bounded and returns to text in one undo step", async () => {
    const largePath = path.join(sandboxRoot, "large.csv");
    writeLargeCsv(largePath, 100_001);
    await invokeInApp("e2e_open_path", { path: largePath });
    await waitForEditorText((text) => text.startsWith("id,name,value"), 30_000);
    const originalStart = (await readEditorText()).slice(0, 80);

    await enterCsvTable();
    await waitForCsvInfo("100001 rows");
    const renderedRows = await browser.execute(() =>
      document.querySelectorAll("[data-testid='csv-table'] [data-row]").length,
    );
    expect(renderedRows).toBeLessThanOrEqual(200 * 4);

    await editCsvCell(0, 1, "large-edit");
    await exitCsvTable();
    await waitForEditorText((text) => text.includes("large-edit"), 30_000);

    await focusEditor();
    await pressMod("z");
    await waitForEditorText((text) => text.slice(0, 80) === originalStart, 30_000);
  });
});
