import { tick } from "svelte";
import type { ColumnDef } from "@tanstack/svelte-table";
import type { useCsvHistory, TableOp, HistoryEntry } from "./useCsvHistory.svelte";
import type { HotkeyBinding } from "$lib/hotkeys";

type CsvHistory = ReturnType<typeof useCsvHistory>;

type ParsedCsv = {
    headers: string[];
    rows: string[][];
    delimiter: string;
    errors: string[];
};

export function useCsvEditorState(
    history: CsvHistory,
    getParsed: () => ParsedCsv,
    setParsed: (p: ParsedCsv) => void,
    columns: () => ColumnDef<string[], string>[],
    scrollToIndex: (index: number, options?: { align?: "auto" | "start" | "center" | "end" }) => void,
    onOpsApplied?: (ops: TableOp[], reverse: boolean) => void,
) {
    let editingCell = $state<{ rowIndex: number; colIndex: number } | null>(null);
    let focusedCell = $state<{ rowIndex: number; colIndex: number } | null>(null);
    let editValue = $state("");

    function getCellValue(rowIndex: number, colIndex: number): string {
        if (rowIndex === -1) {
            return getParsed().headers[colIndex] ?? "";
        }
        return getParsed().rows[rowIndex]?.[colIndex] ?? "";
    }

    /** Apply a list of operations forward (for redo and normal edits) */
    function applyOps(ops: TableOp[]) {
        const parsed = getParsed();
        let rowChange = false;
        let headerChange = false;

        for (const op of ops) {
            switch (op.type) {
                case 'cell': {
                    const rowArr = parsed.rows[op.row];
                    if (rowArr) {
                        const newRow = [...rowArr];
                        while (newRow.length <= op.col) {
                            newRow.push("");
                        }
                        newRow[op.col] = op.newValue;
                        parsed.rows[op.row] = newRow;
                        rowChange = true;
                    }
                    break;
                }
                case 'header-cell': {
                    parsed.headers[op.col] = op.newValue;
                    headerChange = true;
                    break;
                }
                case 'row-add':
                    parsed.rows.splice(op.index, 0, [...op.data]);
                    rowChange = true;
                    break;
                case 'row-delete':
                    parsed.rows.splice(op.index, 1);
                    rowChange = true;
                    break;
            }
        }

        if (rowChange || headerChange) {
            setParsed({
                ...parsed,
                rows: rowChange ? [...parsed.rows] : parsed.rows,
                headers: headerChange ? [...parsed.headers] : parsed.headers
            });
        }

        onOpsApplied?.(ops, false);
    }

    /** Reverse a list of operations (for undo) */
    function reverseOps(ops: TableOp[]) {
        const parsed = getParsed();
        let rowChange = false;
        let headerChange = false;

        // Apply in reverse order
        for (let i = ops.length - 1; i >= 0; i--) {
            const op = ops[i];
            switch (op.type) {
                case 'cell': {
                    const rowArr = parsed.rows[op.row];
                    if (rowArr) {
                        const newRow = [...rowArr];
                        while (newRow.length <= op.col) {
                            newRow.push("");
                        }
                        newRow[op.col] = op.oldValue;
                        parsed.rows[op.row] = newRow;
                        rowChange = true;
                    }
                    break;
                }
                case 'header-cell': {
                    parsed.headers[op.col] = op.oldValue;
                    headerChange = true;
                    break;
                }
                case 'row-add':
                    parsed.rows.splice(op.index, 1);
                    rowChange = true;
                    break;
                case 'row-delete':
                    parsed.rows.splice(op.index, 0, [...op.data]);
                    rowChange = true;
                    break;
            }
        }

        if (rowChange || headerChange) {
            setParsed({
                ...parsed,
                rows: rowChange ? [...parsed.rows] : parsed.rows,
                headers: headerChange ? [...parsed.headers] : parsed.headers
            });
        }

        onOpsApplied?.(ops, true);
    }

    function handleUndo() {
        const entry = history.undo();
        if (!entry) return;
        reverseOps(entry);
    }

    function handleRedo() {
        const entry = history.redo();
        if (!entry) return;
        applyOps(entry);
    }

    function startEditing(rowIndex: number, colIndex: number, value: string) {
        editingCell = { rowIndex, colIndex };
        editValue = value;
        tick().then(() => {
            const input = document.querySelector(".csv-edit-input") as HTMLInputElement;
            input?.focus();
            input?.select();
        });
    }

    function pushAndApply(op: TableOp) {
        history.push(op);
        applyOps([op]);
    }

    function commitEdit() {
        if (!editingCell) return;
        const { rowIndex, colIndex } = editingCell;
        const oldValue = getCellValue(rowIndex, colIndex);

        if (oldValue !== editValue) {
            const op: TableOp = rowIndex === -1
                ? { type: 'header-cell', col: colIndex, oldValue, newValue: editValue }
                : { type: 'cell', row: rowIndex, col: colIndex, oldValue, newValue: editValue };
            pushAndApply(op);
        }
        editingCell = null;
    }

    function cancelEdit() {
        editingCell = null;
    }

    function clearCell() {
        if (!focusedCell) return;
        const oldValue = getCellValue(focusedCell.rowIndex, focusedCell.colIndex);
        if (oldValue !== "") {
            const op: TableOp = focusedCell.rowIndex === -1
                ? { type: 'header-cell', col: focusedCell.colIndex, oldValue, newValue: "" }
                : { type: 'cell', row: focusedCell.rowIndex, col: focusedCell.colIndex, oldValue, newValue: "" };
            pushAndApply(op);
        }
    }

    function handleTabNavigation(isShift: boolean) {
        if (!focusedCell) return;
        let nextRow = focusedCell.rowIndex;
        let nextCol = focusedCell.colIndex + (isShift ? -1 : 1);
        const parsed = getParsed();
        const cols = columns();

        if (nextCol >= cols.length) {
            nextCol = 0;
            nextRow = Math.min(parsed.rows.length - 1, nextRow + 1);
        } else if (nextCol < 0) {
            nextCol = cols.length - 1;
            nextRow = Math.max(-1, nextRow - 1);
        }
        focusedCell = { rowIndex: nextRow, colIndex: nextCol };
    }

    function focusCurrentCell() {
        if (!focusedCell) return;
        tick().then(() => {
            const cell = document.querySelector(
                `[data-row="${focusedCell!.rowIndex}"][data-col="${focusedCell!.colIndex}"]`,
            ) as HTMLElement;
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
        { key: "Enter", callback: () => {
            const parsed = getParsed();
            commitEdit();
            if (focusedCell) {
                focusedCell = {
                    rowIndex: Math.min(parsed.rows.length - 1, focusedCell.rowIndex + 1),
                    colIndex: focusedCell.colIndex,
                };
            }
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: false } },
        { key: "Escape", callback: () => {
            cancelEdit();
            focusCurrentCell();
        }, options: { preventDefault: true, ignoreInputs: false } },
        { key: "Tab", callback: () => {
            commitEdit();
            if (focusedCell) {
                handleTabNavigation(false);
            }
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: false } },
        { key: "Shift+Tab", callback: () => {
            commitEdit();
            if (focusedCell) {
                handleTabNavigation(true);
            }
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: false } }
    ];

    const PAGE_SIZE = 20;

    const cellHotkeys: HotkeyBinding[] = [
        { key: "Mod+Z", callback: handleUndo, options: { preventDefault: true, ignoreInputs: true } },
        { key: "Mod+Shift+Z", callback: handleRedo, options: { preventDefault: true, ignoreInputs: true } },
        { key: "Mod+Y", callback: handleRedo, options: { preventDefault: true, ignoreInputs: true } },
        { key: "Enter", callback: () => {
            if (!focusedCell) return;
            startEditing(focusedCell.rowIndex, focusedCell.colIndex, getCellValue(focusedCell.rowIndex, focusedCell.colIndex));
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "F2", callback: () => {
            if (!focusedCell) return;
            startEditing(focusedCell.rowIndex, focusedCell.colIndex, getCellValue(focusedCell.rowIndex, focusedCell.colIndex));
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "Delete", callback: () => {
            if (!focusedCell) return;
            clearCell();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "Backspace", callback: () => {
            if (!focusedCell) return;
            clearCell();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "ArrowUp", callback: () => {
            if (!focusedCell) return;
            focusedCell = { rowIndex: Math.max(-1, focusedCell.rowIndex - 1), colIndex: focusedCell.colIndex };
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "ArrowDown", callback: () => {
            if (!focusedCell) return;
            const maxRow = getParsed().rows.length - 1;
            focusedCell = { rowIndex: Math.min(maxRow, focusedCell.rowIndex + 1), colIndex: focusedCell.colIndex };
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "ArrowLeft", callback: () => {
            if (!focusedCell) return;
            focusedCell = { rowIndex: focusedCell.rowIndex, colIndex: Math.max(0, focusedCell.colIndex - 1) };
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "ArrowRight", callback: () => {
            if (!focusedCell) return;
            const maxCol = columns().length - 1;
            focusedCell = { rowIndex: focusedCell.rowIndex, colIndex: Math.min(maxCol, focusedCell.colIndex + 1) };
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "Tab", callback: () => {
            if (!focusedCell) return;
            handleTabNavigation(false);
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "Shift+Tab", callback: () => {
            if (!focusedCell) return;
            handleTabNavigation(true);
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "Home", callback: () => {
            if (!focusedCell) return;
            focusedCell = { rowIndex: focusedCell.rowIndex, colIndex: 0 };
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "Mod+Home" as any, callback: () => {
            if (!focusedCell) return;
            focusedCell = { rowIndex: 0, colIndex: 0 };
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "End", callback: () => {
            if (!focusedCell) return;
            focusedCell = { rowIndex: focusedCell.rowIndex, colIndex: columns().length - 1 };
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "Mod+End" as any, callback: () => {
            if (!focusedCell) return;
            focusedCell = { rowIndex: getParsed().rows.length - 1, colIndex: columns().length - 1 };
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "PageUp", callback: () => {
            if (!focusedCell) return;
            focusedCell = { rowIndex: Math.max(0, focusedCell.rowIndex - PAGE_SIZE), colIndex: focusedCell.colIndex };
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "PageDown", callback: () => {
            if (!focusedCell) return;
            const maxRow = getParsed().rows.length - 1;
            focusedCell = { rowIndex: Math.min(maxRow, focusedCell.rowIndex + PAGE_SIZE), colIndex: focusedCell.colIndex };
            navigateAndFocus();
        }, options: { preventDefault: true, ignoreInputs: true } },
        { key: "Escape", callback: () => {
            focusedCell = null;
        }, options: { preventDefault: true, ignoreInputs: true } }
    ];

    function handleCellKeydown(e: KeyboardEvent) {
        if (editingCell) return;
        if (!focusedCell) return;

        const { rowIndex, colIndex } = focusedCell;

        // Type-to-edit: single printable character starts editing
        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
            e.preventDefault();
            editingCell = { rowIndex, colIndex };
            editValue = e.key;
            tick().then(() => {
                const input = document.querySelector(".csv-edit-input") as HTMLInputElement;
                if (input) {
                    input.focus();
                    input.selectionStart = input.selectionEnd = input.value.length;
                }
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
        set focusedCell(val) {
            focusedCell = val;
        },
        get editValue() {
            return editValue;
        },
        set editValue(val) {
            editValue = val;
        },
        startEditing,
        commitEdit,
        cancelEdit,
        cellHotkeys,
        editHotkeys,
        handleCellKeydown,
        navigateAndFocus,
    };
}
