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

export type TableOp = CellEdit | RowAdd | RowDelete | HeaderEdit | BulkRowDelete | BulkRowAdd | BulkColDelete | BulkColAdd | BulkCellClear;

export type WorkerOp = 
    | { type: 'cell'; row: number; col: number; value: string }
    | { type: 'header-cell'; col: number; value: string }
    | { type: 'row-add'; index: number; data: string[][] }
    | { type: 'row-delete'; index: number; count: number }
    | { type: 'col-add'; index: number; headers: string[]; data: string[][] }
    | { type: 'col-delete'; index: number; count: number }
    | { type: 'cell-clear'; startRow: number; endRow: number; startCol: number; endCol: number }
    | { type: 'cell-fill'; startRow: number; endRow: number; startCol: number; endCol: number; data: string[][] };

export function translateToWorkerOps(ops: TableOp[], reverse: boolean): WorkerOp[] {
    const result: WorkerOp[] = [];
    const opsArray = reverse ? [...ops].reverse() : ops;

    for (const op of opsArray) {
        switch (op.type) {
            case 'cell':
                result.push({ type: 'cell', row: op.row, col: op.col, value: reverse ? op.oldValue : op.newValue });
                break;
            case 'header-cell':
                result.push({ type: 'header-cell', col: op.col, value: reverse ? op.oldValue : op.newValue });
                break;
            case 'row-add':
                if (reverse) result.push({ type: 'row-delete', index: op.index, count: 1 });
                else result.push({ type: 'row-add', index: op.index, data: [op.data] });
                break;
            case 'row-delete':
                if (reverse) result.push({ type: 'row-add', index: op.index, data: [op.data] });
                else result.push({ type: 'row-delete', index: op.index, count: 1 });
                break;
            case 'bulk-row-delete':
                if (reverse) result.push({ type: 'row-add', index: op.start, data: op.data });
                else result.push({ type: 'row-delete', index: op.start, count: op.end - op.start + 1 });
                break;
            case 'bulk-row-add':
                if (reverse) result.push({ type: 'row-delete', index: op.start, count: op.data.length });
                else result.push({ type: 'row-add', index: op.start, data: op.data });
                break;
            case 'bulk-col-delete':
                if (reverse) result.push({ type: 'col-add', index: op.start, headers: op.headers, data: op.data });
                else result.push({ type: 'col-delete', index: op.start, count: op.end - op.start + 1 });
                break;
            case 'bulk-col-add':
                if (reverse) result.push({ type: 'col-delete', index: op.start, count: op.headers.length });
                else result.push({ type: 'col-add', index: op.start, headers: op.headers, data: op.data });
                break;
            case 'bulk-cell-clear':
                if (reverse) result.push({ type: 'cell-fill', startRow: op.startRow, endRow: op.endRow, startCol: op.startCol, endCol: op.endCol, data: op.oldValues });
                else result.push({ type: 'cell-clear', startRow: op.startRow, endRow: op.endRow, startCol: op.startCol, endCol: op.endCol });
                break;
        }
    }
    return result;
}

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
