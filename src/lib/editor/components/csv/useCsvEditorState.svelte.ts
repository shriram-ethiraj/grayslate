import { tick } from "svelte";
import type { ColumnDef } from "@tanstack/svelte-table";
import type { HotkeyBinding } from "$lib/hotkeys";
import {
    completeEditorLoader,
    hideEditorLoader,
    startLoaderTicker,
    stopLoaderTicker,
} from "$lib/state/editor.svelte";
import type {
    CsvMutationRequest,
    CsvSelectionBlock,
    CsvTableController,
    CsvTableSnapshot,
} from "./csvTableProtocol";

type SelectionBlock = CsvSelectionBlock;

type MutationLoaderConfig = {
    message: string;
    subMessage: string;
};

const LARGE_TABLE_MUTATION_ROW_THRESHOLD = 200_000;
const LARGE_SELECTION_CLEAR_CELL_THRESHOLD = 5_000;
const PAGE_SIZE = 20;

export function useCsvEditorState(
    controller: CsvTableController,
    columns: () => ColumnDef<string[], string>[],
    scrollToIndex: (index: number, options?: { align?: "auto" | "start" | "center" | "end" }) => void,
) {
    let editingCell = $state<{ rowIndex: number; colIndex: number } | null>(null);
    let focusedCell = $state<{ rowIndex: number; colIndex: number } | null>(null);
    let selectionBlock = $state<SelectionBlock>(null);
    let isSelecting = $state(false);
    let editValue = $state("");
    let isMutationInFlight = false;

    function getSnapshot(): CsvTableSnapshot {
        return controller.getSnapshot();
    }

    function getRowCount(): number {
        return getSnapshot().rowCount;
    }

    function getCellValue(rowIndex: number, colIndex: number): string {
        return controller.getCachedCellValue(rowIndex, colIndex);
    }

    function getSelectionCellCount(block: SelectionBlock): number {
        if (!block) return 0;
        return (block.endRow - block.startRow + 1) * (block.endCol - block.startCol + 1);
    }

    function getMutationLoaderConfig(userEvent: string, block?: SelectionBlock): MutationLoaderConfig | null {
        const snapshot = getSnapshot();
        if (snapshot.rowCount < LARGE_TABLE_MUTATION_ROW_THRESHOLD) {
            return null;
        }

        const subMessage = `${snapshot.rowCount.toLocaleString()} rows`;

        if (userEvent === "delete.table.column") return { message: "Deleting column…", subMessage };
        if (userEvent === "delete.table.row") return { message: "Deleting row…", subMessage };
        if (userEvent === "input.table.column-add") return { message: "Adding column…", subMessage };
        if (userEvent === "input.table.row-add") return { message: "Adding row…", subMessage };
        if (userEvent === "move.table.row") return { message: "Moving rows…", subMessage };
        if (userEvent === "undo.table") return { message: "Undoing table change…", subMessage };
        if (userEvent === "redo.table") return { message: "Redoing table change…", subMessage };
        if (
            userEvent === "delete.table.selection" &&
            block &&
            getSelectionCellCount(block) >= LARGE_SELECTION_CLEAR_CELL_THRESHOLD
        ) {
            return { message: "Clearing selection…", subMessage };
        }

        return null;
    }

    async function runAsyncAction(
        userEvent: string,
        action: () => Promise<boolean | void>,
        block?: SelectionBlock,
    ): Promise<void> {
        if (isMutationInFlight) return;

        const loaderConfig = getMutationLoaderConfig(userEvent, block);
        isMutationInFlight = true;

        if (loaderConfig) {
            startLoaderTicker(loaderConfig.message, loaderConfig.subMessage, {
                ceiling: 94,
                factor: 0.04,
                minStep: 0.2,
                interval: 90,
            });
        }

        try {
            const applied = await action();
            if (loaderConfig) {
                if (applied === false) {
                    stopLoaderTicker();
                    hideEditorLoader();
                } else {
                    completeEditorLoader("Table updated", loaderConfig.subMessage, 120);
                }
            }
        } catch (error) {
            stopLoaderTicker();
            hideEditorLoader();
            throw error;
        } finally {
            isMutationInFlight = false;
        }
    }

    function isEntireRowSelection(block: SelectionBlock = selectionBlock): boolean {
        return !!block && block.startCol === 0 && block.endCol >= columns().length - 1;
    }

    function isEntireColumnSelection(block: SelectionBlock = selectionBlock): boolean {
        return !!block && block.startRow === 0 && block.endRow >= Math.max(0, getRowCount() - 1);
    }

    function getSelectedRowRange(): { start: number; end: number } | null {
        if (isEntireRowSelection()) return { start: selectionBlock!.startRow, end: selectionBlock!.endRow };
        if (focusedCell && focusedCell.rowIndex >= 0) return { start: focusedCell.rowIndex, end: focusedCell.rowIndex };
        return null;
    }

    function getSelectedColumnRange(): { start: number; end: number } | null {
        if (isEntireColumnSelection()) return { start: selectionBlock!.startCol, end: selectionBlock!.endCol };
        if (focusedCell && focusedCell.colIndex >= 0) return { start: focusedCell.colIndex, end: focusedCell.colIndex };
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

    function startEditingFocusedCell() {
        if (!focusedCell) return;
        const cell = { ...focusedCell };
        const cachedValue = getCellValue(cell.rowIndex, cell.colIndex);

        if (cachedValue !== "" || cell.rowIndex === -1) {
            startEditing(cell.rowIndex, cell.colIndex, cachedValue);
            return;
        }

        void controller.fetchCellValue(cell.rowIndex, cell.colIndex).then((value) => {
            if (
                !focusedCell ||
                focusedCell.rowIndex !== cell.rowIndex ||
                focusedCell.colIndex !== cell.colIndex
            ) {
                return;
            }
            startEditing(cell.rowIndex, cell.colIndex, value);
        }).catch((error) => {
            console.error("Failed to fetch CSV cell value", error);
        });
    }

    function commitEdit() {
        if (!editingCell) return;

        const cell = editingCell;
        const nextValue = editValue;
        const oldValue = getCellValue(cell.rowIndex, cell.colIndex);
        editingCell = null;

        if (oldValue === nextValue) return;

        const mutation: CsvMutationRequest =
            cell.rowIndex === -1
                ? { type: "edit-header", colIndex: cell.colIndex, newValue: nextValue }
                : {
                      type: "edit-cell",
                      rowIndex: cell.rowIndex,
                      colIndex: cell.colIndex,
                      newValue: nextValue,
                  };

        void runAsyncAction("input.table", () => controller.runMutation(mutation, "input.table"));
    }

    function cancelEdit() {
        editingCell = null;
    }

    function clearCell() {
        if (!focusedCell) return;
        if (getCellValue(focusedCell.rowIndex, focusedCell.colIndex) === "") return;

        const cell = { ...focusedCell };
        void runAsyncAction("delete.table.cell", () =>
            controller.runMutation(
                cell.rowIndex === -1
                    ? { type: "edit-header", colIndex: cell.colIndex, newValue: "" }
                    : { type: "clear-cell", rowIndex: cell.rowIndex, colIndex: cell.colIndex },
                "delete.table.cell",
            ),
        );
    }

    function deleteSelection() {
        if (!selectionBlock) return;

        const block = selectionBlock;

        if (isEntireRowSelection(block)) {
            selectionBlock = null;
            focusedCell = null;
            void runAsyncAction(
                "delete.table.row",
                () => controller.runMutation({ type: "delete-rows", start: block.startRow, end: block.endRow }, "delete.table.row"),
                block,
            );
            return;
        }

        if (isEntireColumnSelection(block)) {
            selectionBlock = null;
            focusedCell = null;
            void runAsyncAction(
                "delete.table.column",
                () => controller.runMutation({ type: "delete-columns", start: block.startCol, end: block.endCol }, "delete.table.column"),
                block,
            );
            return;
        }

        void runAsyncAction(
            "delete.table.selection",
            () =>
                controller.runMutation(
                    {
                        type: "clear-selection",
                        startRow: block.startRow,
                        endRow: block.endRow,
                        startCol: block.startCol,
                        endCol: block.endCol,
                    },
                    "delete.table.selection",
                ),
            block,
        );
    }

    function addRowAt(index: number) {
        focusedCell = { rowIndex: index, colIndex: 0 };
        selectionBlock = {
            startRow: index,
            endRow: index,
            startCol: 0,
            endCol: Math.max(0, columns().length - 1),
        };
        navigateAndFocus();
        void runAsyncAction("input.table.row-add", () =>
            controller.runMutation({ type: "add-row", index }, "input.table.row-add"),
        );
    }

    function addRowAbove() {
        const range = getSelectedRowRange();
        addRowAt(range ? range.start : getRowCount());
    }

    function addRowBelow() {
        const range = getSelectedRowRange();
        addRowAt(range ? range.end + 1 : getRowCount());
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
        focusedCell = { rowIndex: -1, colIndex: index };
        selectionBlock = {
            startRow: 0,
            endRow: Math.max(0, getRowCount() - 1),
            startCol: index,
            endCol: index,
        };
        navigateAndFocus();
        void runAsyncAction("input.table.column-add", () =>
            controller.runMutation({ type: "add-column", index }, "input.table.column-add"),
        );
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
            endRow: Math.max(0, getRowCount() - 1),
            startCol: range.start,
            endCol: range.end,
        };
        deleteSelection();
    }

    function moveSelectedRows(direction: -1 | 1) {
        const range = getSelectedRowRange();
        if (!range) return;

        const count = range.end - range.start + 1;
        const targetStart = range.start + direction;
        if (targetStart < 0 || targetStart + count > getRowCount()) return;

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

        void runAsyncAction("move.table.row", () =>
            controller.runMutation(
                { type: "move-rows", start: range.start, end: range.end, direction },
                "move.table.row",
            ),
        );
    }

    function handleUndo() {
        void runAsyncAction("undo.table", () => controller.undo());
    }

    function handleRedo() {
        void runAsyncAction("redo.table", () => controller.redo());
    }

    function handleTabNavigation(isShift: boolean) {
        if (!focusedCell) return;

        let nextRow = focusedCell.rowIndex;
        let nextCol = focusedCell.colIndex + (isShift ? -1 : 1);
        const currentColumns = columns();

        if (nextCol >= currentColumns.length) {
            nextCol = 0;
            nextRow = Math.min(getRowCount() - 1, nextRow + 1);
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
                commitEdit();
                if (focusedCell) {
                    focusedCell = {
                        rowIndex: Math.min(getRowCount() - 1, focusedCell.rowIndex + 1),
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
                if (focusedCell) handleTabNavigation(false);
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: false },
        },
        {
            key: "Shift+Tab",
            callback: () => {
                commitEdit();
                if (focusedCell) handleTabNavigation(true);
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: false },
        },
    ];

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
            callback: startEditingFocusedCell,
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "F2",
            callback: startEditingFocusedCell,
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Delete",
            callback: () => {
                if (selectionBlock) deleteSelection();
                else if (focusedCell) clearCell();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Backspace",
            callback: () => {
                if (selectionBlock) deleteSelection();
                else if (focusedCell) clearCell();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "ArrowUp",
            callback: () => {
                if (!focusedCell) return;
                selectionBlock = null;
                focusedCell = { rowIndex: Math.max(-1, focusedCell.rowIndex - 1), colIndex: focusedCell.colIndex };
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
                const maxRow = getRowCount() - 1;
                focusedCell = { rowIndex: Math.min(maxRow, focusedCell.rowIndex + 1), colIndex: focusedCell.colIndex };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Shift+ArrowDown",
            callback: () => {
                if (!focusedCell) return;
                const maxRow = getRowCount() - 1;
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
                focusedCell = { rowIndex: focusedCell.rowIndex, colIndex: Math.max(0, focusedCell.colIndex - 1) };
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
                focusedCell = { rowIndex: focusedCell.rowIndex, colIndex: Math.min(maxCol, focusedCell.colIndex + 1) };
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
                focusedCell = { rowIndex: focusedCell.rowIndex, colIndex: columns().length - 1 };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "Mod+End" as const,
            callback: () => {
                if (!focusedCell) return;
                focusedCell = { rowIndex: getRowCount() - 1, colIndex: columns().length - 1 };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "PageUp",
            callback: () => {
                if (!focusedCell) return;
                focusedCell = { rowIndex: Math.max(0, focusedCell.rowIndex - PAGE_SIZE), colIndex: focusedCell.colIndex };
                navigateAndFocus();
            },
            options: { preventDefault: true, ignoreInputs: true },
        },
        {
            key: "PageDown",
            callback: () => {
                if (!focusedCell) return;
                const maxRow = getRowCount() - 1;
                focusedCell = { rowIndex: Math.min(maxRow, focusedCell.rowIndex + PAGE_SIZE), colIndex: focusedCell.colIndex };
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
            editingCell = { rowIndex: focusedCell.rowIndex, colIndex: focusedCell.colIndex };
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