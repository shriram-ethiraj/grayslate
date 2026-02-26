import { tick } from "svelte";
import type { ColumnDef } from "@tanstack/svelte-table";
import type { useCsvHistory, TableOp, HistoryEntry } from "./useCsvHistory.svelte";

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
        let structuralChange = false;

        for (const op of ops) {
            switch (op.type) {
                case 'cell': {
                    const rowArr = parsed.rows[op.row];
                    if (rowArr) {
                        while (rowArr.length <= op.col) {
                            rowArr.push("");
                        }
                        rowArr[op.col] = op.newValue;
                    }
                    break;
                }
                case 'header-cell': {
                    parsed.headers[op.col] = op.newValue;
                    structuralChange = true;
                    break;
                }
                case 'row-add':
                    parsed.rows.splice(op.index, 0, [...op.data]);
                    structuralChange = true;
                    break;
                case 'row-delete':
                    parsed.rows.splice(op.index, 1);
                    structuralChange = true;
                    break;
            }
        }

        if (structuralChange) {
            setParsed({ ...parsed, rows: [...parsed.rows], headers: [...parsed.headers] });
        }
    }

    /** Reverse a list of operations (for undo) */
    function reverseOps(ops: TableOp[]) {
        const parsed = getParsed();
        let structuralChange = false;

        // Apply in reverse order
        for (let i = ops.length - 1; i >= 0; i--) {
            const op = ops[i];
            switch (op.type) {
                case 'cell': {
                    const rowArr = parsed.rows[op.row];
                    if (rowArr) {
                        while (rowArr.length <= op.col) {
                            rowArr.push("");
                        }
                        rowArr[op.col] = op.oldValue;
                    }
                    break;
                }
                case 'header-cell': {
                    parsed.headers[op.col] = op.oldValue;
                    structuralChange = true;
                    break;
                }
                case 'row-add':
                    parsed.rows.splice(op.index, 1);
                    structuralChange = true;
                    break;
                case 'row-delete':
                    parsed.rows.splice(op.index, 0, [...op.data]);
                    structuralChange = true;
                    break;
            }
        }

        if (structuralChange) {
            setParsed({ ...parsed, rows: [...parsed.rows], headers: [...parsed.headers] });
        }
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

    function commitEdit() {
        if (!editingCell) return;
        const { rowIndex, colIndex } = editingCell;
        const oldValue = getCellValue(rowIndex, colIndex);

        if (oldValue !== editValue) {
            if (rowIndex === -1) {
                const op: TableOp = {
                    type: 'header-cell',
                    col: colIndex,
                    oldValue,
                    newValue: editValue,
                };
                history.push(op);
                applyOps([op]);
            } else {
                const op: TableOp = {
                    type: 'cell',
                    row: rowIndex,
                    col: colIndex,
                    oldValue,
                    newValue: editValue,
                };
                history.push(op);
                applyOps([op]);
            }
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
            if (focusedCell.rowIndex === -1) {
                const op: TableOp = {
                    type: 'header-cell',
                    col: focusedCell.colIndex,
                    oldValue,
                    newValue: "",
                };
                history.push(op);
                applyOps([op]);
            } else {
                const op: TableOp = {
                    type: 'cell',
                    row: focusedCell.rowIndex,
                    col: focusedCell.colIndex,
                    oldValue,
                    newValue: "",
                };
                history.push(op);
                applyOps([op]);
            }
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

    function handleEditKeydown(e: KeyboardEvent) {
        if (!editingCell) return;
        e.stopPropagation();

        const parsed = getParsed();
        if (e.key === "Enter") {
            e.preventDefault();
            commitEdit();
            if (focusedCell) {
                focusedCell = {
                    rowIndex: Math.min(parsed.rows.length - 1, focusedCell.rowIndex + 1),
                    colIndex: focusedCell.colIndex,
                };
            }
            navigateAndFocus();
        } else if (e.key === "Escape") {
            cancelEdit();
            focusCurrentCell();
        } else if (e.key === "Tab") {
            e.preventDefault();
            commitEdit();
            if (focusedCell) {
                handleTabNavigation(e.shiftKey);
            }
            navigateAndFocus();
        }
    }

    const PAGE_SIZE = 20;

    function handleCellKeydown(e: KeyboardEvent) {
        if (editingCell) return;

        // Handle Undo / Redo
        if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
            e.preventDefault();
            if (e.shiftKey) {
                handleRedo();
            } else {
                handleUndo();
            }
            return;
        }

        if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "y") {
            e.preventDefault();
            handleRedo();
            return;
        }

        if (!focusedCell) return;

        const { rowIndex, colIndex } = focusedCell;
        const parsed = getParsed();
        const maxRow = parsed.rows.length - 1;
        const maxCol = columns().length - 1;

        switch (e.key) {
            case "Enter":
            case "F2":
                e.preventDefault();
                startEditing(rowIndex, colIndex, getCellValue(rowIndex, colIndex));
                return;

            case "Delete":
            case "Backspace":
                e.preventDefault();
                clearCell();
                return;

            case "ArrowUp":
                e.preventDefault();
                focusedCell = { rowIndex: Math.max(-1, rowIndex - 1), colIndex };
                break;

            case "ArrowDown":
                e.preventDefault();
                focusedCell = { rowIndex: Math.min(maxRow, rowIndex + 1), colIndex };
                break;

            case "ArrowLeft":
                e.preventDefault();
                focusedCell = { rowIndex, colIndex: Math.max(0, colIndex - 1) };
                break;

            case "ArrowRight":
                e.preventDefault();
                focusedCell = { rowIndex, colIndex: Math.min(maxCol, colIndex + 1) };
                break;

            case "Tab":
                e.preventDefault();
                handleTabNavigation(e.shiftKey);
                break;

            case "Home":
                e.preventDefault();
                if (e.ctrlKey || e.metaKey) {
                    focusedCell = { rowIndex: 0, colIndex: 0 };
                } else {
                    focusedCell = { rowIndex, colIndex: 0 };
                }
                break;

            case "End":
                e.preventDefault();
                if (e.ctrlKey || e.metaKey) {
                    focusedCell = { rowIndex: maxRow, colIndex: maxCol };
                } else {
                    focusedCell = { rowIndex, colIndex: maxCol };
                }
                break;

            case "PageUp":
                e.preventDefault();
                focusedCell = { rowIndex: Math.max(0, rowIndex - PAGE_SIZE), colIndex };
                break;

            case "PageDown":
                e.preventDefault();
                focusedCell = { rowIndex: Math.min(maxRow, rowIndex + PAGE_SIZE), colIndex };
                break;

            case "Escape":
                e.preventDefault();
                focusedCell = null;
                return;

            default:
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
                    return;
                }
                return;
        }

        navigateAndFocus();
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
        handleEditKeydown,
        handleCellKeydown,
        navigateAndFocus,
    };
}
