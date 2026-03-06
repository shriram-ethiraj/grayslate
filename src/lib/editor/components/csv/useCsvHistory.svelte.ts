// Structural history for CSV table mode.
// Stores lightweight cell/row operations instead of full CSV strings.

export type CellEdit = {
    type: 'cell';
    row: number;
    col: number;
    oldValue: string;
    newValue: string;
};

export type RowAdd = {
    type: 'row-add';
    index: number;
    data: string[];
};

export type RowDelete = {
    type: 'row-delete';
    index: number;
    data: string[];
};

export type HeaderEdit = {
    type: 'header-cell';
    col: number;
    oldValue: string;
    newValue: string;
};

export type BulkRowDelete = { type: 'bulk-row-delete'; start: number; end: number; data: string[][] };
export type BulkRowAdd = { type: 'bulk-row-add'; start: number; data: string[][] };
export type BulkColDelete = { type: 'bulk-col-delete'; start: number; end: number; headers: string[]; data: string[][] };
export type BulkColAdd = { type: 'bulk-col-add'; start: number; headers: string[]; data: string[][] };
export type BulkCellClear = { type: 'bulk-cell-clear'; startRow: number; endRow: number; startCol: number; endCol: number; oldValues: string[][] };
export type BulkCellFill = { type: 'bulk-cell-fill'; startRow: number; endRow: number; startCol: number; endCol: number; data: string[][] };

export type TableOp = CellEdit | RowAdd | RowDelete | HeaderEdit | BulkRowDelete | BulkRowAdd | BulkColDelete | BulkColAdd | BulkCellClear | BulkCellFill;

// A single undo step can contain multiple operations (e.g. multi-cell paste)
export type HistoryEntry = TableOp[];

const MAX_HISTORY = 200;

export function useCsvHistory() {
    let undoStack = $state<HistoryEntry[]>([]);
    let redoStack = $state<HistoryEntry[]>([]);
    let dirty = $state(false);

    /** Push a batch of operations as a single undo step */
    function push(ops: TableOp | TableOp[]) {
        const entry = Array.isArray(ops) ? ops : [ops];
        if (entry.length === 0) return;

        undoStack.push(entry);

        // Cap history size
        if (undoStack.length > MAX_HISTORY) {
            undoStack.splice(0, undoStack.length - MAX_HISTORY);
        }

        redoStack = [];
        dirty = true;
    }

    /** Undo the last entry. Returns the ops to reverse, or null if nothing to undo. */
    function undo(): HistoryEntry | null {
        if (undoStack.length === 0) return null;
        const entry = undoStack.pop()!;
        redoStack.push(entry);
        dirty = true;
        return entry;
    }

    /** Redo the last undone entry. Returns the ops to re-apply, or null. */
    function redo(): HistoryEntry | null {
        if (redoStack.length === 0) return null;
        const entry = redoStack.pop()!;
        undoStack.push(entry);
        dirty = true;
        return entry;
    }

    /** Reset history (e.g. when switching modes) */
    function clear() {
        undoStack = [];
        redoStack = [];
        dirty = false;
    }

    return {
        get canUndo() {
            return undoStack.length > 0;
        },
        get canRedo() {
            return redoStack.length > 0;
        },
        get isDirty() {
            return dirty;
        },
        set isDirty(val: boolean) {
            dirty = val;
        },
        push,
        undo,
        redo,
        clear,
    };
}
