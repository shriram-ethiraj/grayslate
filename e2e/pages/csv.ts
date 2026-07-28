import { $, browser } from "@wdio/globals";
import { TIMEOUTS, INTERVALS } from "../config/timeouts.js";
import {
  ALT,
  ARROW_DOWN,
  ARROW_RIGHT,
  ARROW_UP,
  DELETE,
  ENTER,
  ESCAPE,
  SHIFT,
  TAB,
  pressMod,
  typeText,
} from "../driver/keys.js";
import { countElements, readCellText } from "../driver/probe.js";
import { waitFor } from "../driver/wait.js";
import {
  byTestId,
  clickTestId,
  isDisplayedTestId,
  textOf,
  waitForTestId,
} from "./common.js";

/**
 * CSV table mode.
 *
 * Every interaction here is a real click or keystroke. The previous helpers
 * synthesized a `dblclick`, assigned `input.value` directly, and called `blur()`
 * inside a `requestAnimationFrame` — which meant the commit path a user
 * actually exercises (typing, then Enter/Tab/click-away) was never tested.
 */

export async function enter(): Promise<void> {
  await clickTestId("action-table-view");
  await waitForTestId("csv-table", { timeoutMs: TIMEOUTS.heavy });
}

/** Start table initialization without polling WebDriver concurrently. */
export async function requestEnter(): Promise<void> {
  await clickTestId("action-table-view");
}

export async function exit(): Promise<void> {
  await clickTestId("action-plain-csv");
  await waitForTestId("csv-table", { reverse: true, timeoutMs: TIMEOUTS.heavy });
}

/** A grid cell at (row, col). Column -1 is the row-number gutter. */
export function cell(row: number, col: number): ReturnType<typeof $> {
  return $(`[data-row='${row}'][data-col='${col}']`);
}

export async function cellText(row: number, col: number): Promise<string> {
  return (await readCellText(row, col)) ?? "";
}

export async function waitForCellText(
  row: number,
  col: number,
  expected: string,
): Promise<void> {
  let observed = "";
  await waitFor(
    async () => {
      observed = await cellText(row, col);
      return observed === expected;
    },
    {
      message: () => `Cell (${row}, ${col}) never became ${JSON.stringify(expected)}. Last: ${JSON.stringify(observed)}`,
      timeoutMs: TIMEOUTS.ui,
      intervalMs: INTERVALS.fast,
    },
  );
}

export async function selectCell(row: number, col: number): Promise<void> {
  const target = await cell(row, col);
  await target.waitForClickable({
    timeout: TIMEOUTS.ui,
    timeoutMsg: `Cell (${row}, ${col}) never became clickable.`,
  });
  await target.click();
}

export async function isCellSelected(row: number, col: number): Promise<boolean> {
  return (await (await cell(row, col)).getAttribute("aria-selected")) === "true";
}

/**
 * Edit a cell the way a user does: double-click to open the inline editor, type
 * the value, then commit with Enter.
 */
export async function editCell(
  row: number,
  col: number,
  value: string,
  commit: "enter" | "tab" | "escape" = "enter",
): Promise<void> {
  // Select first: the cell's own click handler focuses the grid, and the
  // double-click handler assumes that has happened.
  await selectCell(row, col);

  const target = await cell(row, col);
  await target.doubleClick();

  // Re-resolve the input from the document rather than from the captured cell
  // handle: committing swaps the cell's contents between a `<div>` and an
  // `<input>`, so a handle taken before the double-click is stale by the time
  // the editor exists.
  // Wait for the editor to mount. Checking existence immediately after the
  // double-click races the re-render and reports "not found" for an editor that
  // is about to appear.
  const selector = `[data-row='${row}'][data-col='${col}'] input`;
  const input = await $(selector);
  await input.waitForExist({
    timeout: TIMEOUTS.ui,
    timeoutMsg: `The inline editor for cell (${row}, ${col}) never opened after a double-click.`,
  });

  // Type straight into the already-focused editor rather than using
  // `setValue`. `setValue` clears by clicking the element first, and the cell
  // editor commits on blur — so that click commits the edit and unmounts the
  // input before a single character is typed.
  await pressMod("a");
  await typeText(value);

  if (commit === "enter") await browser.keys(ENTER);
  else if (commit === "tab") await browser.keys(TAB);
  else await browser.keys(ESCAPE);
}

export async function clearSelection(): Promise<void> {
  await browser.keys(DELETE);
}

/** Select every cell in the table, the way Mod+A does in the grid. */
export async function selectAllCells(): Promise<void> {
  await pressMod("a");
}

/** Copy the current selection as CSV. */
export async function copySelection(): Promise<void> {
  await pressMod("c");
}

/**
 * Open the grid's context menu for a whole row, by right-clicking its gutter.
 *
 * The menu opens only when the selection is a complete row or a complete
 * column — `CsvTableView` checks `isRowSelection() || isColumnSelection()`
 * before calling `openMenu`. Right-clicking an ordinary cell produces a 1×1
 * selection block, which is neither, so the menu correctly never appears: its
 * items are all row and column operations. Targeting the row-number gutter
 * (`data-col="-1"`) is what a user does to act on a row.
 */
export async function openRowContextMenu(row: number): Promise<void> {
  const gutter = await cell(row, -1);
  await gutter.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: `The row gutter for row ${row} never rendered.`,
  });
  await gutter.click({ button: "right" });
  await waitForTestId("csv-context-menu");
}

/** Open the context menu for a whole column, by right-clicking its header. */
export async function openColumnContextMenu(col: number): Promise<void> {
  const header = await cell(-1, col);
  await header.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: `The header for column ${col} never rendered.`,
  });
  await header.click({ button: "right" });
  await waitForTestId("csv-context-menu");
}

export async function isContextMenuOpen(): Promise<boolean> {
  return isDisplayedTestId("csv-context-menu");
}

export async function chooseContextMenuItem(
  item:
    | "insert-column-left"
    | "insert-column-right"
    | "insert-row-above"
    | "insert-row-below"
    | "move-row-up"
    | "move-row-down"
    | "move-column-left"
    | "move-column-right"
    | "delete-row"
    | "delete-column",
): Promise<void> {
  await clickTestId(`csv-context-${item}`);
}

export async function moveRowUp(): Promise<void> {
  await browser.keys([ALT, ARROW_UP]);
}

export async function moveRowDown(): Promise<void> {
  await browser.keys([ALT, ARROW_DOWN]);
}

export async function info(): Promise<string> {
  return textOf("status-csv-info");
}

export async function waitForInfo(
  predicate: (text: string) => boolean,
  message: string,
): Promise<void> {
  let observed = "";
  await waitFor(
    async () => {
      observed = await info();
      return predicate(observed);
    },
    {
      message: () => `${message} Last observed: ${JSON.stringify(observed)}`,
      timeoutMs: TIMEOUTS.heavy,
      intervalMs: INTERVALS.slow,
    },
  );
}

/**
 * How many row elements the virtualizer has actually rendered.
 *
 * The safety contract for the >100k-row path: the DOM must stay bounded no
 * matter how large the document is.
 */
export async function renderedRowCount(): Promise<number> {
  return countElements("[data-testid='csv-table'] [data-row]");
}

export async function isTableVisible(): Promise<boolean> {
  return (await byTestId("csv-table")).isDisplayed().catch(() => false);
}
