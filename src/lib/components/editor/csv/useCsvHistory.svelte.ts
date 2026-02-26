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

export type TableOp = CellEdit | RowAdd | RowDelete | HeaderEdit;

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
