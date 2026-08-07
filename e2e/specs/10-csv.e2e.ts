import fs from "node:fs";
import path from "node:path";
import { browser, expect } from "@wdio/globals";
import { TIMEOUTS } from "../config/timeouts.js";
import { expectFileBytes } from "../assertions/matchers.js";
import { scenario } from "../coverage/scenario.js";
import { ARROW_DOWN, ARROW_RIGHT } from "../driver/keys.js";
import {
  armOperationGate,
  releaseOperationGateOutOfProcess,
  waitForOperationGate,
  waitForOperationSignalOutOfProcess,
} from "../driver/operationGate.js";
import { waitForFile, waitForFileBytes } from "../driver/wait.js";
import {
  externalRoot,
  openBytes,
  openPath,
  openText,
  writeLargeCsv,
} from "../fixtures/factories.js";
import { waitForClipboardText } from "../driver/clipboard.js";
import { clickTestId, isAriaDisabled } from "../pages/common.js";
import * as csv from "../pages/csv.js";
import * as editor from "../pages/editor.js";
import * as statusBar from "../pages/statusBar.js";

/**
 * CSV table mode.
 *
 * Cell edits go through a real double-click, real typing, and a real Enter
 * commit. The previous version set `input.value` directly and called `blur()`
 * inside a `requestAnimationFrame`, which meant the commit path a user actually
 * takes was never exercised.
 */
const SAMPLE = "id,name,city\n1,Alice,Paris\n2,Bob,London\n3,Carol,Berlin\n";

describe("CSV table mode", () => {
  async function openTable(name: string, body = SAMPLE): Promise<string> {
    const filePath = await openText(name, body);
    await csv.enter();
    return filePath;
  }

  scenario(
    "csv.enter-table",
    "reports the row, column, and delimiter counts on entering table view",
    async () => {
      await openTable("enter-table.csv");

      await csv.waitForInfo(
        (text) => text.includes("3 rows"),
        "Table mode did not report three data rows.",
      );
      const info = await csv.info();
      expect(info).toContain("3 cols");
      expect(info.toLowerCase()).toContain("comma");
    },
  );

  scenario(
    "csv.navigate-and-select",
    "moves the selected cell with the arrow keys",
    async () => {
      await openTable("navigate.csv");

      await csv.selectCell(0, 0);
      expect(await csv.isCellSelected(0, 0)).toBe(true);

      await browser.keys(ARROW_DOWN);
      await csv.waitForCellText(1, 0, "2");
      expect(await csv.isCellSelected(1, 0)).toBe(true);

      await browser.keys(ARROW_RIGHT);
      expect(await csv.isCellSelected(1, 1)).toBe(true);
    },
  );

  scenario(
    "csv.edit-cell",
    "commits a typed cell value with Enter",
    async () => {
      await openTable("edit-cell.csv");

      await csv.editCell(0, 1, "Alice Edited");
      await csv.waitForCellText(0, 1, "Alice Edited");
    },
  );

  scenario("csv.clear-cell", "empties a cell without removing its row", async () => {
    await openTable("clear-cell.csv");

    await csv.selectCell(0, 1);
    await csv.clearSelection();
    await csv.waitForCellText(0, 1, "");

    // The row survives; only the cell was cleared.
    await csv.waitForCellText(0, 0, "1");
    await csv.waitForInfo(
      (text) => text.includes("3 rows"),
      "Clearing a cell changed the row count.",
    );
  });

  scenario("csv.undo-redo", "undoes and redoes a table edit", async () => {
    await openTable("undo-redo.csv");

    await csv.editCell(0, 1, "Changed");
    await csv.waitForCellText(0, 1, "Changed");

    // Undo is surface-aware: in table mode it drives the Rust CSV session
    // history rather than CodeMirror.
    await editor.undo();
    await csv.waitForCellText(0, 1, "Alice");

    await editor.redo();
    await csv.waitForCellText(0, 1, "Changed");
  });

  scenario("csv.rows.insert-delete", "inserts rows above and below and deletes one", async () => {
    await openTable("rows-insert-delete.csv");

    await csv.openRowContextMenu(1);
    await csv.chooseContextMenuItem("insert-row-above");
    await csv.waitForInfo(
      (text) => text.includes("4 rows"),
      "Inserting a row above did not change the row count.",
    );
    // The new row lands between Alice and Bob, so Alice keeps row 0.
    await csv.waitForCellText(0, 1, "Alice");
    await csv.waitForCellText(1, 1, "");

    await csv.openRowContextMenu(1);
    await csv.chooseContextMenuItem("insert-row-below");
    await csv.waitForInfo(
      (text) => text.includes("5 rows"),
      "Inserting a row below did not change the row count.",
    );

    await csv.openRowContextMenu(1);
    await csv.chooseContextMenuItem("delete-row");
    await csv.waitForInfo(
      (text) => text.includes("4 rows"),
      "Deleting a row did not change the row count.",
    );
  });

  scenario(
    "csv.columns.insert-delete",
    "inserts columns left and right and deletes one",
    async () => {
      await openTable("columns-insert-delete.csv");

      await csv.openColumnContextMenu(1);
      await csv.chooseContextMenuItem("insert-column-left");
      await csv.waitForInfo(
        (text) => text.includes("4 cols"),
        "Inserting a column left did not change the column count.",
      );
      // The new empty column takes index 1 and pushes "Alice" right.
      await csv.waitForCellText(0, 1, "");
      await csv.waitForCellText(0, 2, "Alice");

      await csv.openColumnContextMenu(1);
      await csv.chooseContextMenuItem("insert-column-right");
      await csv.waitForInfo(
        (text) => text.includes("5 cols"),
        "Inserting a column right did not change the column count.",
      );

      await csv.openColumnContextMenu(1);
      await csv.chooseContextMenuItem("delete-column");
      await csv.waitForInfo(
        (text) => text.includes("4 cols"),
        "Deleting a column did not change the column count.",
      );
    },
  );

  scenario("csv.columns.move", "moves a column and the new order is visible", async () => {
    await openTable("move-column.csv");

    await csv.openColumnContextMenu(1);
    await csv.chooseContextMenuItem("move-column-right");

    // "Alice" was in column 1 and "Paris" in column 2; moving right swaps them.
    await csv.waitForCellText(0, 1, "Paris");
    await csv.waitForCellText(0, 2, "Alice");

    await csv.openColumnContextMenu(2);
    await csv.chooseContextMenuItem("move-column-left");
    await csv.waitForCellText(0, 1, "Alice");
    await csv.waitForCellText(0, 2, "Paris");
  });

  scenario(
    "csv.context-menu",
    "opens the grid context menu and performs a row operation from it",
    async () => {
      await openTable("context-menu.csv");

      await csv.openRowContextMenu(0);
      // The menu is the surface under test, so assert it actually opened before
      // acting: a silently-absent menu would otherwise look like a failed edit.
      expect(await csv.isContextMenuOpen()).toBe(true);

      await csv.chooseContextMenuItem("insert-row-below");
      await csv.waitForInfo(
        (text) => text.includes("4 rows"),
        "The context menu's insert-row-below did nothing.",
      );
      expect(await csv.isContextMenuOpen()).toBe(false);
    },
  );

  scenario(
    "csv.copy",
    "copies correctly escaped CSV, committing an open edit first",
    async () => {
      await openTable("copy.csv", "id,name\n1,Alice\n");

      // A value containing the delimiter must be quoted on the way out, and the
      // edit must be committed by the copy rather than lost.
      await csv.editCell(0, 1, "Smith, Alice");
      await csv.waitForCellText(0, 1, "Smith, Alice");

      await csv.selectAllCells();
      await csv.copySelection();
      await waitForClipboardText('id,name\n1,"Smith, Alice"');
    },
  );

  scenario("csv.save", "writes the edited rows to disk from table mode", async () => {
    const filePath = await openTable("save-from-table.csv", "id,name\n1,Alice\n");

    await csv.editCell(0, 1, "Edited");
    await csv.waitForCellText(0, 1, "Edited");

    await editor.save();
    await waitForFile(filePath, (content) => content.includes("Edited"), {
      message: "Saving from table mode never reached disk.",
    });
    // Table mode serializes without re-adding the source file's trailing
    // newline, so assert the bytes the app actually writes rather than the
    // bytes it was given.
    expectFileBytes(filePath, "id,name\n1,Edited");
  });

  scenario(
    "csv.small.live-mirror-history",
    "keeps each small-table edit as an individual text-mode undo step",
    async () => {
      const original = "id,name\n1,Alice\n2,Bob\n";
      await openTable("round-trip-undo.csv", original);

      // Three separate table edits, which must collapse to one text-mode step.
      await csv.editCell(0, 1, "One");
      await csv.waitForCellText(0, 1, "One");
      await csv.editCell(1, 1, "Two");
      await csv.waitForCellText(1, 1, "Two");
      await csv.editCell(0, 0, "9");
      await csv.waitForCellText(0, 0, "9");

      await csv.exit();
      await editor.waitForExactText("id,name\n9,One\n2,Two");

      // Below the 100k-row mirroring threshold every table edit is mirrored
      // into CodeMirror's history as its own step, so undo walks back through
      // them one at a time rather than collapsing. The single-step collapse is
      // the *large*-session contract and is covered by
      // `csv.large.mirroring-threshold`.
      await editor.focus();
      await editor.undo();
      await editor.waitForExactText("id,name\n1,One\n2,Two");
      await editor.undo();
      await editor.undo();
      await editor.waitForExactText(original);
    },
  );

  scenario("csv.rows.move", "moves a row and the new order is visible", async () => {
    await openTable("move-row.csv");

    await csv.selectCell(0, 0);
    await csv.moveRowDown();

    // Alice's row must now sit below Bob's — asserted on the grid, not merely
    // that the keystroke was accepted. The previous suite pressed Alt+ArrowDown
    // and never checked anything at all.
    await csv.waitForCellText(0, 1, "Bob");
    await csv.waitForCellText(1, 1, "Alice");
  });

  scenario(
    "csv.format-preservation",
    "preserves encoding and line endings through an edited table save",
    async () => {
      const crlf = "id,name\r\n1,Alice\r\n2,Bob\r\n";
      const filePath = await openBytes(
        "format-preserve.csv",
        Buffer.from(`\uFEFF${crlf}`, "utf16le"),
      );
      await statusBar.waitForEol("crlf");
      await statusBar.waitForEncoding("utf-16le");

      await csv.enter();
      await csv.editCell(0, 1, "Edited");
      await editor.save();
      const expectedBytes = Buffer.from(
        "\uFEFFid,name\r\n1,Edited\r\n2,Bob",
        "utf16le",
      );
      await waitForFileBytes(filePath, (bytes) => bytes.equals(expectedBytes), {
        message: "The UTF-16LE/CRLF table save did not reach disk.",
      });
      expectFileBytes(
        filePath,
        expectedBytes,
      );
      await csv.exit();
      await statusBar.waitForEol("crlf");
      await statusBar.waitForEncoding("utf-16le");
    },
  );

  scenario(
    "csv.actions-unavailable",
    "disables text-only transformations in table mode",
    async () => {
      await openTable("unavailable.csv");

      // Table mode owns the surface; text-oriented actions must be disabled
      // rather than silently doing nothing.
      expect(await isAriaDisabled("action-transformations")).toBe(true);
    },
  );

  scenario(
    "csv.flexible-rows",
    "keeps uneven CSV rows usable instead of aborting the table",
    async () => {
      await openTable(
        "flexible-rows.csv",
        "id,name\n1,Alice\n2\n3,Carol,extra\n",
      );
      await csv.waitForInfo(
        (text) => text.includes("3 rows") && text.includes("2 cols"),
        "Uneven CSV rows aborted the flexible table parse.",
      );
      await csv.waitForCellText(1, 0, "2");
      await csv.waitForCellText(1, 1, "");
    },
  );

  scenario(
    "csv.large.bounded-render",
    "keeps the rendered DOM bounded for a 100k-row file",
    async () => {
      const filePath = path.join(externalRoot, "large.csv");
      fs.mkdirSync(externalRoot, { recursive: true });
      writeLargeCsv(filePath, 100_001);

      await openPath(filePath);
      await csv.enter();

      await csv.waitForInfo(
        (text) => text.includes("100001 rows"),
        "The large CSV did not report its full row count.",
      );

      // The virtualizer's whole purpose: the DOM stays small no matter how
      // large the document is.
      expect(await csv.renderedRowCount()).toBeLessThan(800);
    },
  );

  scenario(
    "csv.large.mirroring-threshold",
    "returns a >100k-row session to text mode in one undo step",
    async () => {
      const filePath = path.join(externalRoot, "large-threshold.csv");
      fs.mkdirSync(externalRoot, { recursive: true });
      writeLargeCsv(filePath, 100_001);

      await openPath(filePath);
      await csv.enter();
      await csv.editCell(0, 1, "row-1-edited");
      await csv.waitForCellText(0, 1, "row-1-edited");

      // Above the live-mirroring threshold the session is not mirrored into
      // CodeMirror history as it goes, so leaving must still produce exactly
      // one undoable step rather than none.
      await csv.exit();
      await editor.waitForText(
        (text) => text.includes("row-1-edited"),
        "The large table session did not carry its edit back to text mode.",
        TIMEOUTS.heavy,
      );

      await editor.focus();
      await editor.undo();
      await editor.waitForText(
        (text) => !text.includes("row-1-edited"),
        "A large table session did not undo as a single step.",
        TIMEOUTS.heavy,
      );
    },
  );

  scenario(
    "csv.delimiters",
    "recognizes semicolon and tab delimited files",
    async () => {
      await openTable("semicolons.csv", "id;name;city\n1;Alice;Paris\n2;Bob;London\n");

      await csv.waitForInfo(
        (text) => text.includes("2 rows"),
        "The semicolon-delimited file did not parse into rows.",
      );
      const info = (await csv.info()).toLowerCase();
      expect(info).toContain("semicolon");
      // Columns must split on the detected delimiter, not fall back to one.
      expect(info).toContain("3 cols");

      await csv.exit();
      await openTable("tabs.tsv", "id\tname\tcity\n1\tAlice\tParis\n2\tBob\tLondon\n");
      await csv.waitForInfo(
        (text) => text.toLowerCase().includes("tab") && text.includes("3 cols"),
        "The tab-delimited file was not detected as a three-column table.",
      );
    },
  );

  scenario(
    "csv.cancel",
    "cancels an in-flight table initialization when the document changes",
    async () => {
      await openText("csv-cancel.csv", SAMPLE);
      await armOperationGate("csv-initialize");
      await csv.requestEnter();
      await waitForOperationGate("csv-initialize");
      await armOperationGate("csv-dispose");

      const newSlateRequest = clickTestId("header-new-slate");
      let observationError: unknown;
      try {
        // Replacing the document unmounts CsvTableView; its teardown invokes
        // csv_cancel and disposes the session before the held parse can return.
        await waitForOperationSignalOutOfProcess("csv-dispose");
      } catch (error) {
        observationError = error;
      } finally {
        releaseOperationGateOutOfProcess("csv-initialize");
      }
      await newSlateRequest;
      if (observationError) throw observationError;

      await editor.waitUntilReady({ documentPath: "New Slate", documentLength: 0 });
      expect(await csv.isTableVisible()).toBe(false);
      await editor.replaceText("usable after CSV cancellation");
    },
  );
});
