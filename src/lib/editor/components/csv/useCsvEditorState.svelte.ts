import { tick } from "svelte";
import type { ColumnDef } from "@tanstack/svelte-table";
import type { CsvParseResult } from "$lib/editor/core/csvParser";
import { serializeCsv } from "$lib/editor/core/csvParser";
import { dispatchCsvDocChange } from "$lib/editor/core/csvCodeMirror";
import { editorState as appEditorState } from "$lib/state/editor.svelte";
import type { HotkeyBinding } from "$lib/hotkeys";
import { useCsvHistory, type TableOp } from "./useCsvHistory.svelte";

type ParsedCsv = CsvParseResult;

type SelectionBlock = {
    startRow: number;
    endRow: number;
    startCol: number;
    endCol: number;
} | null;

function cloneParsed(parsed: ParsedCsv): ParsedCsv {
    return {
        headers: [...parsed.headers],
        rows: parsed.rows.map((row) => [...row]),
        delimiter: parsed.delimiter,
        errors: [...parsed.errors],
    };
}

function applyOpsToParsed(parsed: ParsedCsv, ops: TableOp[]): ParsedCsv {
    const nextParsed = cloneParsed(parsed);

    for (const op of ops) {
        switch (op.type) {
            case "cell": {
                const rowArr = nextParsed.rows[op.row];
                if (!rowArr) break;
                while (rowArr.length <= op.col) {
                    rowArr.push("");
                }
                rowArr[op.col] = op.newValue;
                break;
            }
            case "header-cell": {
                while (nextParsed.headers.length <= op.col) {
                    nextParsed.headers.push("");
                }
                nextParsed.headers[op.col] = op.newValue;
                break;
            }
            case "row-add":
                nextParsed.rows.splice(op.index, 0, [...op.data]);
                break;
            case "row-delete":
                nextParsed.rows.splice(op.index, 1);
                break;
            case "bulk-row-delete":
                nextParsed.rows.splice(op.start, op.end - op.start + 1);
                break;
            case "bulk-row-add":
                nextParsed.rows.splice(op.start, 0, ...op.data.map((row) => [...row]));
                break;
            case "bulk-col-delete": {
                nextParsed.headers.splice(op.start, op.end - op.start + 1);
                nextParsed.rows = nextParsed.rows.map((row) => {
                    const nextRow = [...row];
                    nextRow.splice(op.start, op.end - op.start + 1);
                    return nextRow;
                });
                break;
            }
            case "bulk-col-add": {
                nextParsed.headers.splice(op.start, 0, ...op.headers);
                nextParsed.rows = nextParsed.rows.map((row, index) => {
                    const rowData = op.data[index] ?? [];
                    return [
                        ...row.slice(0, op.start),
                        ...rowData,
                        ...row.slice(op.start),
                    ];
                });
                break;
            }
            case "bulk-cell-clear": {
                for (let rowIndex = op.startRow; rowIndex <= op.endRow; rowIndex += 1) {
                    const row = nextParsed.rows[rowIndex];
                    if (!row) continue;
                    for (let colIndex = op.startCol; colIndex <= op.endCol; colIndex += 1) {
                        while (row.length <= colIndex) {
                            row.push("");
                        }
                        row[colIndex] = "";
                    }
                }
                break;
            }
        }
    }

    return nextParsed;
}

function invertOp(op: TableOp): TableOp {
    switch (op.type) {
        case "cell":
            return { ...op, oldValue: op.newValue, newValue: op.oldValue };
        case "header-cell":
            return { ...op, oldValue: op.newValue, newValue: op.oldValue };
        case "row-add":
            return { type: "row-delete", index: op.index, data: [...op.data] };
        case "row-delete":
            return { type: "row-add", index: op.index, data: [...op.data] };
        case "bulk-row-delete":
            return { type: "bulk-row-add", start: op.start, data: op.data.map((row) => [...row]) };
        case "bulk-row-add":
            return {
                type: "bulk-row-delete",
                start: op.start,
                end: op.start + op.data.length - 1,
                data: op.data.map((row) => [...row]),
            };
        case "bulk-col-delete":
            return {
                type: "bulk-col-add",
                start: op.start,
                headers: [...op.headers],
                data: op.data.map((row) => [...row]),
            };
        case "bulk-col-add":
            return {
                type: "bulk-col-delete",
                start: op.start,
                end: op.start + op.headers.length - 1,
                headers: [...op.headers],
                data: op.data.map((row) => [...row]),
            };
        case "bulk-cell-clear":
            return {
                type: "bulk-cell-clear",
                startRow: op.startRow,
                endRow: op.endRow,
                startCol: op.startCol,
                endCol: op.endCol,
                oldValues: op.oldValues.map((row) => [...row]),
            };
    }
}

function invertOps(ops: TableOp[]): TableOp[] {
    return [...ops].reverse().map(invertOp);
}

export function useCsvEditorState(
    history: ReturnType<typeof useCsvHistory>,
    getParsed: () => ParsedCsv,
    setParsed: (parsed: ParsedCsv) => void,
    columns: () => ColumnDef<string[], string>[],
    scrollToIndex: (index: number, options?: { align?: "auto" | "start" | "center" | "end" }) => void,
    onCsvTextApplied?: (nextText: string, userEvent: string) => void,
) {
    let editingCell = $state<{ rowIndex: number; colIndex: number } | null>(null);
    let focusedCell = $state<{ rowIndex: number; colIndex: number } | null>(null);
    let selectionBlock = $state<SelectionBlock>(null);
    let isSelecting = $state(false);
    let editValue = $state("");

    function getCellValue(rowIndex: number, colIndex: number): string {
        if (rowIndex === -1) {
            return getParsed().headers[colIndex] ?? "";
        }
        return getParsed().rows[rowIndex]?.[colIndex] ?? "";
    }

    function commitParsedChange(nextParsed: ParsedCsv, userEvent: string) {
        const nextText = serializeCsv(
            nextParsed.headers,
            nextParsed.rows,
            nextParsed.delimiter,
        );

        setParsed(nextParsed);
        onCsvTextApplied?.(nextText, userEvent);

        const view = appEditorState.activeView;
        if (!view) {
            return;
        }

        dispatchCsvDocChange(view, nextText, {
            userEvent,
            focus: false,
        });
    }

    function applyTableOps(ops: TableOp[], userEvent: string, options?: { pushHistory?: boolean }) {
        if (ops.length === 0) return;
        if (options?.pushHistory !== false) {
            history.push(ops);
        }
        const nextParsed = applyOpsToParsed(getParsed(), ops);
        commitParsedChange(nextParsed, userEvent);
    }

    function isEntireRowSelection(block: SelectionBlock = selectionBlock): boolean {
        return !!block && block.startCol === 0 && block.endCol >= columns().length - 1;
    }

    function isEntireColumnSelection(block: SelectionBlock = selectionBlock): boolean {
        return (
            !!block &&
            block.startRow === 0 &&
            block.endRow >= Math.max(0, getParsed().rows.length - 1)
        );
    }

    function getSelectedRowRange(): { start: number; end: number } | null {
        if (isEntireRowSelection()) {
            return {
                start: selectionBlock!.startRow,
                end: selectionBlock!.endRow,
            };
        }
        if (focusedCell && focusedCell.rowIndex >= 0) {
            return { start: focusedCell.rowIndex, end: focusedCell.rowIndex };
        }
        return null;
    }

    function getSelectedColumnRange(): { start: number; end: number } | null {
        if (isEntireColumnSelection()) {
            return {
                start: selectionBlock!.startCol,
                end: selectionBlock!.endCol,
            };
        }
        if (focusedCell && focusedCell.colIndex >= 0) {
            return { start: focusedCell.colIndex, end: focusedCell.colIndex };
        }
        return null;
    }

    function startEditing(
        rowIndex: number,
        colIndex: number,
        value: string,
        options?: { selectAll?: boolean; cursorPosition?: number },
    ) {
        editingCell = { rowIndex, colIndex };
        editValue = value;
        tick().then(() => {
            const input = document.querySelector(".csv-edit-input") as HTMLInputElement | null;
            if (!input) return;
            input.focus();
            if (options?.selectAll) {
                input.select();
                return;
            }
            if (options?.cursorPosition !== undefined) {
                const pos = Math.max(0, Math.min(input.value.length, options.cursorPosition));
                input.selectionStart = pos;
                input.selectionEnd = pos;
                return;
            }
            input.selectionStart = input.value.length;
            input.selectionEnd = input.value.length;
        });
    }

    function commitEdit() {
        if (!editingCell) return;

        const { rowIndex, colIndex } = editingCell;
        const oldValue = getCellValue(rowIndex, colIndex);
        editingCell = null;

        if (oldValue === editValue) {
            return;
        }

        applyTableOps(
            [
                rowIndex === -1
                    ? {
                        type: "header-cell",
                        col: colIndex,
                        oldValue,
                        newValue: editValue,
                    }
                    : {
                        type: "cell",
                        row: rowIndex,
                        col: colIndex,
                        oldValue,
                        newValue: editValue,
                    },
            ],
            "input.table",
        );
    }

    function cancelEdit() {
        editingCell = null;
    }

    function clearCell() {
        if (!focusedCell) return;

        const oldValue = getCellValue(focusedCell.rowIndex, focusedCell.colIndex);
        if (oldValue === "") {
            return;
        }

        applyTableOps(
            [
                focusedCell.rowIndex === -1
                    ? {
                        type: "header-cell",
                        col: focusedCell.colIndex,
                        oldValue,
                        newValue: "",
                    }
                    : {
                        type: "cell",
                        row: focusedCell.rowIndex,
                        col: focusedCell.colIndex,
                        oldValue,
                        newValue: "",
                    },
            ],
            "delete.table.cell",
        );
    }

    function deleteSelection() {
        if (!selectionBlock) return;

        const parsed = getParsed();
        const { startRow, endRow, startCol, endCol } = selectionBlock;
        const numRows = parsed.rows.length;

        if (isEntireRowSelection()) {
            const deletedRows = parsed.rows
                .slice(startRow, endRow + 1)
                .map((row) => [...row]);
            applyTableOps(
                [
                    {
                        type: "bulk-row-delete",
                        start: startRow,
                        end: endRow,
                        data: deletedRows,
                    },
                ],
                "delete.table.row",
            );
            selectionBlock = null;
            focusedCell = null;
            return;
        }

        if (isEntireColumnSelection()) {
            const deletedHeaders = parsed.headers.slice(startCol, endCol + 1);
            const deletedData = parsed.rows.map((row) => row.slice(startCol, endCol + 1));
            applyTableOps(
                [
                    {
                        type: "bulk-col-delete",
                        start: startCol,
                        end: endCol,
                        headers: deletedHeaders,
                        data: deletedData,
                    },
                ],
                "delete.table.column",
            );
            selectionBlock = null;
            focusedCell = null;
            return;
        }

        const oldValues: string[][] = [];
        for (let rowIndex = startRow; rowIndex <= endRow; rowIndex += 1) {
            const row = parsed.rows[rowIndex];
            const oldRow: string[] = [];
            if (row) {
                for (let colIndex = startCol; colIndex <= endCol; colIndex += 1) {
                    oldRow.push(row[colIndex] ?? "");
                }
            }
            oldValues.push(oldRow);
        }

        const clearOps: TableOp[] = [];
        for (let rowIndex = startRow; rowIndex <= endRow; rowIndex += 1) {
            for (let colIndex = startCol; colIndex <= endCol; colIndex += 1) {
                const oldValue = parsed.rows[rowIndex]?.[colIndex] ?? "";
                if (oldValue === "") continue;
                clearOps.push({
                    type: "cell",
                    row: rowIndex,
                    col: colIndex,
                    oldValue,
                    newValue: "",
                });
            }
        }

        if (clearOps.length === 0) {
            return;
        }

        applyTableOps(clearOps, "delete.table.selection");
    }

    function addRowAt(index: number) {
        const width = Math.max(getParsed().headers.length, columns().length);
        applyTableOps(
            [
                {
                    type: "row-add",
                    index,
                    data: Array.from({ length: width }, () => ""),
                },
            ],
            "input.table.row-add",
        );
        focusedCell = { rowIndex: index, colIndex: 0 };
        selectionBlock = {
            startRow: index,
            endRow: index,
            startCol: 0,
            endCol: Math.max(0, columns().length - 1),
        };
        navigateAndFocus();
    }

    function addRowAbove() {
        const range = getSelectedRowRange();
        addRowAt(range ? range.start : getParsed().rows.length);
    }

    function addRowBelow() {
        const range = getSelectedRowRange();
        addRowAt(range ? range.end + 1 : getParsed().rows.length);
    }

    function deleteSelectedRows() {
        const range = getSelectedRowRange();
        if (!range) return;

        selectionBlock = {
            startRow: range.start,
            endRow: range.end,
            startCol: 0,
            endCol: Math.max(0, columns().length - 1),
        };
        deleteSelection();
    }

    function addColumnAt(index: number) {
        const parsed = getParsed();
        applyTableOps(
            [
                {
                    type: "bulk-col-add",
                    start: index,
                    headers: [""],
                    data: parsed.rows.map(() => [""]),
                },
            ],
            "input.table.column-add",
        );
        focusedCell = { rowIndex: -1, colIndex: index };
        selectionBlock = {
            startRow: 0,
            endRow: Math.max(0, parsed.rows.length - 1),
            startCol: index,
            endCol: index,
        };
        navigateAndFocus();
    }

    function addColumnLeft() {
        const range = getSelectedColumnRange();
        addColumnAt(range ? range.start : columns().length);
    }

    function addColumnRight() {
        const range = getSelectedColumnRange();
        addColumnAt(range ? range.end + 1 : columns().length);
    }

    function deleteSelectedColumns() {
        const range = getSelectedColumnRange();
        if (!range) return;

        selectionBlock = {
            startRow: 0,
            endRow: Math.max(0, getParsed().rows.length - 1),
            startCol: range.start,
            endCol: range.end,
        };
        deleteSelection();
    }

    function moveSelectedRows(direction: -1 | 1) {
        const range = getSelectedRowRange();
        if (!range) return;

        const parsed = cloneParsed(getParsed());
        const count = range.end - range.start + 1;
        const targetStart = range.start + direction;

        if (targetStart < 0 || targetStart + count > parsed.rows.length) {
            return;
        }

        const movedRows = parsed.rows.slice(range.start, range.end + 1);
        applyTableOps(
            [
                {
                    type: "bulk-row-delete",
                    start: range.start,
                    end: range.end,
                    data: movedRows.map((row) => [...row]),
                },
                {
                    type: "bulk-row-add",
                    start: targetStart,
                    data: movedRows.map((row) => [...row]),
                },
            ],
            "move.table.row",
        );

        selectionBlock = {
            startRow: targetStart,
            endRow: targetStart + count - 1,
            startCol: 0,
            endCol: Math.max(0, columns().length - 1),
        };
        focusedCell = {
            rowIndex: targetStart,
            colIndex: focusedCell?.colIndex ?? 0,
        };
        navigateAndFocus();
    }

    function handleUndo() {
        const entry = history.undo();
        if (!entry) return;
        applyTableOps(invertOps(entry), "undo.table", { pushHistory: false });
    }

    function handleRedo() {
        const entry = history.redo();
        if (!entry) return;
        applyTableOps(entry, "redo.table", { pushHistory: false });
    }

    function handleTabNavigation(isShift: boolean) {
        if (!focusedCell) return;

        let nextRow = focusedCell.rowIndex;
        let nextCol = focusedCell.colIndex + (isShift ? -1 : 1);
        const parsed = getParsed();
        const currentColumns = columns();

        if (nextCol >= currentColumns.length) {
            nextCol = 0;
            nextRow = Math.min(parsed.rows.length - 1, nextRow + 1);
        } else if (nextCol < 0) {
            nextCol = currentColumns.length - 1;
            nextRow = Math.max(-1, nextRow - 1);
        }

        focusedCell = { rowIndex: nextRow, colIndex: nextCol };
    }

    function focusCurrentCell() {
        if (!focusedCell) return;

        tick().then(() => {
            const cell = document.querySelector(
                `[data-row="${focusedCell!.rowIndex}"][data-col="${focusedCell!.colIndex}"]`,
            ) as HTMLElement | null;
            cell?.focus();
        });
    }

    function navigateAndFocus() {
        if (!focusedCell) return;
        if (focusedCell.rowIndex >= 0) {
            scrollToIndex(focusedCell.rowIndex, { align: "auto" });
        } else {
            scrollToIndex(0, { align: "start" });
        }
        focusCurrentCell();
    }

    const editHotkeys: HotkeyBinding[] = [
        {
            key: "Enter",
            callback: () => {
                const parsed = getParsed();
                commitEdit();
                if (focusedCell) {
                    focusedCell = {
                        rowIndex: Math.min(parsed.rows.length - 1, focusedCell.rowIndex + 1),
                        colIndex: focusedCell.colIndex,
                    };
                }
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: false },
        },
        {
            key: "Escape",
            callback: () => {
                cancelEdit();
                focusCurrentCell();
            },
            options: { preventDefault: true, ignoreInputs: false },
        },
        {
            key: "Tab",
            callback: () => {
                commitEdit();
                if (focusedCell) {
                    handleTabNavigation(false);
                }
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: false },
        },
        {
            key: "Shift+Tab",
            callback: () => {
                commitEdit();
                if (focusedCell) {
                    handleTabNavigation(true);
                }
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: false },
        },
    ];

    const PAGE_SIZE = 20;

    const cellHotkeys: HotkeyBinding[] = [
        { key: "Mod+Z", callback: handleUndo, options: { preventDefault: true, ignoreInputs: true } },
        { key: "Mod+Shift+Z", callback: handleRedo, options: { preventDefault: true, ignoreInputs: true } },
        { key: "Mod+Y", callback: handleRedo, options: { preventDefault: true, ignoreInputs: true } },
        {
            key: "Alt+ArrowUp",
            callback: () => moveSelectedRows(-1),
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Alt+ArrowDown",
            callback: () => moveSelectedRows(1),
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Enter",
            callback: () => {
                if (!focusedCell) return;
                startEditing(
                    focusedCell.rowIndex,
                    focusedCell.colIndex,
                    getCellValue(focusedCell.rowIndex, focusedCell.colIndex),
                );
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "F2",
            callback: () => {
                if (!focusedCell) return;
                startEditing(
                    focusedCell.rowIndex,
                    focusedCell.colIndex,
                    getCellValue(focusedCell.rowIndex, focusedCell.colIndex),
                );
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Delete",
            callback: () => {
                if (selectionBlock) {
                    deleteSelection();
                } else if (focusedCell) {
                    clearCell();
                }
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Backspace",
            callback: () => {
                if (selectionBlock) {
                    deleteSelection();
                } else if (focusedCell) {
                    clearCell();
                }
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "ArrowUp",
            callback: () => {
                if (!focusedCell) return;
                selectionBlock = null;
                focusedCell = {
                    rowIndex: Math.max(-1, focusedCell.rowIndex - 1),
                    colIndex: focusedCell.colIndex,
                };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Shift+ArrowUp",
            callback: () => {
                if (!focusedCell) return;
                const rowIndex = Math.max(-1, focusedCell.rowIndex - 1);
                if (!selectionBlock) {
                    selectionBlock = {
                        startRow: Math.min(focusedCell.rowIndex, rowIndex),
                        endRow: Math.max(focusedCell.rowIndex, rowIndex),
                        startCol: focusedCell.colIndex,
                        endCol: focusedCell.colIndex,
                    };
                } else {
                    selectionBlock = {
                        ...selectionBlock,
                        startRow: Math.min(selectionBlock.startRow, rowIndex),
                        endRow: Math.max(selectionBlock.endRow, rowIndex),
                    };
                }
                focusedCell = { rowIndex, colIndex: focusedCell.colIndex };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "ArrowDown",
            callback: () => {
                if (!focusedCell) return;
                selectionBlock = null;
                const maxRow = getParsed().rows.length - 1;
                focusedCell = {
                    rowIndex: Math.min(maxRow, focusedCell.rowIndex + 1),
                    colIndex: focusedCell.colIndex,
                };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Shift+ArrowDown",
            callback: () => {
                if (!focusedCell) return;
                const maxRow = getParsed().rows.length - 1;
                const rowIndex = Math.min(maxRow, focusedCell.rowIndex + 1);
                if (!selectionBlock) {
                    selectionBlock = {
                        startRow: Math.min(focusedCell.rowIndex, rowIndex),
                        endRow: Math.max(focusedCell.rowIndex, rowIndex),
                        startCol: focusedCell.colIndex,
                        endCol: focusedCell.colIndex,
                    };
                } else {
                    selectionBlock = {
                        ...selectionBlock,
                        startRow: Math.min(selectionBlock.startRow, rowIndex),
                        endRow: Math.max(selectionBlock.endRow, rowIndex),
                    };
                }
                focusedCell = { rowIndex, colIndex: focusedCell.colIndex };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "ArrowLeft",
            callback: () => {
                if (!focusedCell) return;
                selectionBlock = null;
                focusedCell = {
                    rowIndex: focusedCell.rowIndex,
                    colIndex: Math.max(0, focusedCell.colIndex - 1),
                };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Shift+ArrowLeft",
            callback: () => {
                if (!focusedCell) return;
                const colIndex = Math.max(0, focusedCell.colIndex - 1);
                if (!selectionBlock) {
                    selectionBlock = {
                        startRow: focusedCell.rowIndex,
                        endRow: focusedCell.rowIndex,
                        startCol: Math.min(focusedCell.colIndex, colIndex),
                        endCol: Math.max(focusedCell.colIndex, colIndex),
                    };
                } else {
                    selectionBlock = {
                        ...selectionBlock,
                        startCol: Math.min(selectionBlock.startCol, colIndex),
                        endCol: Math.max(selectionBlock.endCol, colIndex),
                    };
                }
                focusedCell = { rowIndex: focusedCell.rowIndex, colIndex };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "ArrowRight",
            callback: () => {
                if (!focusedCell) return;
                selectionBlock = null;
                const maxCol = columns().length - 1;
                focusedCell = {
                    rowIndex: focusedCell.rowIndex,
                    colIndex: Math.min(maxCol, focusedCell.colIndex + 1),
                };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Shift+ArrowRight",
            callback: () => {
                if (!focusedCell) return;
                const maxCol = columns().length - 1;
                const colIndex = Math.min(maxCol, focusedCell.colIndex + 1);
                if (!selectionBlock) {
                    selectionBlock = {
                        startRow: focusedCell.rowIndex,
                        endRow: focusedCell.rowIndex,
                        startCol: Math.min(focusedCell.colIndex, colIndex),
                        endCol: Math.max(focusedCell.colIndex, colIndex),
                    };
                } else {
                    selectionBlock = {
                        ...selectionBlock,
                        startCol: Math.min(selectionBlock.startCol, colIndex),
                        endCol: Math.max(selectionBlock.endCol, colIndex),
                    };
                }
                focusedCell = { rowIndex: focusedCell.rowIndex, colIndex };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Tab",
            callback: () => {
                if (!focusedCell) return;
                handleTabNavigation(false);
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Shift+Tab",
            callback: () => {
                if (!focusedCell) return;
                handleTabNavigation(true);
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Home",
            callback: () => {
                if (!focusedCell) return;
                focusedCell = { rowIndex: focusedCell.rowIndex, colIndex: 0 };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Mod+Home" as const,
            callback: () => {
                if (!focusedCell) return;
                focusedCell = { rowIndex: 0, colIndex: 0 };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "End",
            callback: () => {
                if (!focusedCell) return;
                focusedCell = {
                    rowIndex: focusedCell.rowIndex,
                    colIndex: columns().length - 1,
                };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Mod+End" as const,
            callback: () => {
                if (!focusedCell) return;
                focusedCell = {
                    rowIndex: getParsed().rows.length - 1,
                    colIndex: columns().length - 1,
                };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "PageUp",
            callback: () => {
                if (!focusedCell) return;
                focusedCell = {
                    rowIndex: Math.max(0, focusedCell.rowIndex - PAGE_SIZE),
                    colIndex: focusedCell.colIndex,
                };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "PageDown",
            callback: () => {
                if (!focusedCell) return;
                const maxRow = getParsed().rows.length - 1;
                focusedCell = {
                    rowIndex: Math.min(maxRow, focusedCell.rowIndex + PAGE_SIZE),
                    colIndex: focusedCell.colIndex,
                };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Escape",
            callback: () => {
                selectionBlock = null;
                focusedCell = null;
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
    ];

    function handleCellKeydown(e: KeyboardEvent) {
        if (editingCell || !focusedCell) return;

        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
            e.preventDefault();
            editingCell = {
                rowIndex: focusedCell.rowIndex,
                colIndex: focusedCell.colIndex,
            };
            editValue = e.key;
            tick().then(() => {
                const input = document.querySelector(".csv-edit-input") as HTMLInputElement | null;
                if (!input) return;
                input.focus();
                input.selectionStart = input.value.length;
                input.selectionEnd = input.value.length;
            });
        }
    }

    return {
        get editingCell() {
            return editingCell;
        },
        get focusedCell() {
            return focusedCell;
        },
        set focusedCell(value) {
            focusedCell = value;
        },
        get editValue() {
            return editValue;
        },
        set editValue(value) {
            editValue = value;
        },
        get selectionBlock() {
            return selectionBlock;
        },
        set selectionBlock(value) {
            selectionBlock = value;
        },
        get isSelecting() {
            return isSelecting;
        },
        set isSelecting(value) {
            isSelecting = value;
        },
        isRowSelection: () => isEntireRowSelection(),
        isColumnSelection: () => isEntireColumnSelection(),
        startEditing,
        commitEdit,
        cancelEdit,
        clearCell,
        deleteSelection,
        addRowAbove,
        addRowBelow,
        deleteSelectedRows,
        addColumnLeft,
        addColumnRight,
        deleteSelectedColumns,
        moveSelectedRowsUp: () => moveSelectedRows(-1),
        moveSelectedRowsDown: () => moveSelectedRows(1),
        handleUndo,
        handleRedo,
        cellHotkeys,
        editHotkeys,
        handleCellKeydown,
        navigateAndFocus,
    };
}