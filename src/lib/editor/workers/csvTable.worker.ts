import Papa from "papaparse";
import { serializeCsv } from "../core/csvParser";
import {
    LIVE_MIRROR_ROW_THRESHOLD,
    type CsvMirrorTextUpdate,
    type CsvMutationRequest,
    type CsvTableSnapshot,
    type CsvWorkerRequest,
    type CsvWorkerResponse,
} from "../components/csv/csvTableProtocol";

type CellEdit = {
    type: "cell";
    row: number;
    col: number;
    oldValue: string;
    newValue: string;
};

type RowAdd = {
    type: "row-add";
    index: number;
    data: string[];
};

type RowDelete = {
    type: "row-delete";
    index: number;
    data: string[];
};

type HeaderEdit = {
    type: "header-cell";
    col: number;
    oldValue: string;
    newValue: string;
};

type BulkRowDelete = {
    type: "bulk-row-delete";
    start: number;
    end: number;
    data: string[][];
};

type BulkRowAdd = {
    type: "bulk-row-add";
    start: number;
    data: string[][];
};

type BulkColDelete = {
    type: "bulk-col-delete";
    start: number;
    end: number;
    headers: string[];
    data: string[][];
};

type BulkColAdd = {
    type: "bulk-col-add";
    start: number;
    headers: string[];
    data: string[][];
};

type BulkCellClear = {
    type: "bulk-cell-clear";
    startRow: number;
    endRow: number;
    startCol: number;
    endCol: number;
    oldValues: string[][];
};

type BulkCellFill = {
    type: "bulk-cell-fill";
    startRow: number;
    endRow: number;
    startCol: number;
    endCol: number;
    data: string[][];
};

type TableOp =
    | CellEdit
    | RowAdd
    | RowDelete
    | HeaderEdit
    | BulkRowDelete
    | BulkRowAdd
    | BulkColDelete
    | BulkColAdd
    | BulkCellClear
    | BulkCellFill;

type CsvState = {
    headers: string[];
    rows: string[][];
    delimiter: string;
    errors: string[];
    text: string;
    version: number;
    serializedVersion: number;
    undoStack: TableOp[][];
    redoStack: TableOp[][];
    liveMirrorEnabled: boolean;
};

const CHUNK_SIZE = 50_000;
const MAX_HISTORY = 200;
const DIRTY_SERIALIZED_VERSION = -1;

const state: CsvState = {
    headers: [],
    rows: [],
    delimiter: ",",
    errors: [],
    text: "",
    version: 0,
    serializedVersion: 0,
    undoStack: [],
    redoStack: [],
    liveMirrorEnabled: false,
};

function postResponse(response: CsvWorkerResponse): void {
    self.postMessage(response);
}

function snapshot(): CsvTableSnapshot {
    return {
        headers: [...state.headers],
        rowCount: state.rows.length,
        delimiter: state.delimiter,
        errors: [...state.errors],
        version: state.version,
        liveMirrorEnabled: state.liveMirrorEnabled,
    };
}

function cloneRows(rows: string[][]): string[][] {
    return rows.map((row) => [...row]);
}

function pushHistory(ops: TableOp[]): void {
    if (ops.length === 0) return;
    state.undoStack.push(ops);
    if (state.undoStack.length > MAX_HISTORY) {
        state.undoStack.splice(0, state.undoStack.length - MAX_HISTORY);
    }
    state.redoStack = [];
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
            return { type: "bulk-row-add", start: op.start, data: cloneRows(op.data) };
        case "bulk-row-add":
            return {
                type: "bulk-row-delete",
                start: op.start,
                end: op.start + op.data.length - 1,
                data: cloneRows(op.data),
            };
        case "bulk-col-delete":
            return {
                type: "bulk-col-add",
                start: op.start,
                headers: [...op.headers],
                data: cloneRows(op.data),
            };
        case "bulk-col-add":
            return {
                type: "bulk-col-delete",
                start: op.start,
                end: op.start + op.headers.length - 1,
                headers: [...op.headers],
                data: cloneRows(op.data),
            };
        case "bulk-cell-clear":
            return {
                type: "bulk-cell-fill",
                startRow: op.startRow,
                endRow: op.endRow,
                startCol: op.startCol,
                endCol: op.endCol,
                data: cloneRows(op.oldValues),
            };
        case "bulk-cell-fill":
            return {
                type: "bulk-cell-clear",
                startRow: op.startRow,
                endRow: op.endRow,
                startCol: op.startCol,
                endCol: op.endCol,
                oldValues: cloneRows(op.data),
            };
    }
}

function invertOps(ops: TableOp[]): TableOp[] {
    return [...ops].reverse().map(invertOp);
}

function ensureRowWidth(row: string[], col: number): void {
    while (row.length <= col) {
        row.push("");
    }
}

type TableModel = {
    headers: string[];
    rows: string[][];
};

function applyOpsToModel(model: TableModel, ops: TableOp[]): void {
    for (const op of ops) {
        switch (op.type) {
            case "cell": {
                const row = model.rows[op.row];
                if (!row) break;
                ensureRowWidth(row, op.col);
                row[op.col] = op.newValue;
                break;
            }
            case "header-cell": {
                while (model.headers.length <= op.col) {
                    model.headers.push("");
                }
                model.headers[op.col] = op.newValue;
                break;
            }
            case "row-add":
                model.rows.splice(op.index, 0, [...op.data]);
                break;
            case "row-delete":
                model.rows.splice(op.index, 1);
                break;
            case "bulk-row-delete":
                model.rows.splice(op.start, op.end - op.start + 1);
                break;
            case "bulk-row-add":
                model.rows.splice(op.start, 0, ...cloneRows(op.data));
                break;
            case "bulk-col-delete": {
                model.headers.splice(op.start, op.end - op.start + 1);
                for (const row of model.rows) {
                    row.splice(op.start, op.end - op.start + 1);
                }
                if (model.headers.length === 0) {
                    model.rows = [];
                }
                break;
            }
            case "bulk-col-add": {
                model.headers.splice(op.start, 0, ...op.headers);
                while (model.rows.length < op.data.length) {
                    model.rows.push([]);
                }
                for (let index = 0; index < model.rows.length; index += 1) {
                    const row = model.rows[index];
                    const rowData = op.data[index] ?? [];
                    row.splice(op.start, 0, ...rowData);
                }
                break;
            }
            case "bulk-cell-clear": {
                for (let rowIndex = op.startRow; rowIndex <= op.endRow; rowIndex += 1) {
                    const row = model.rows[rowIndex];
                    if (!row) continue;
                    for (let colIndex = op.startCol; colIndex <= op.endCol; colIndex += 1) {
                        ensureRowWidth(row, colIndex);
                        row[colIndex] = "";
                    }
                }
                break;
            }
            case "bulk-cell-fill": {
                for (let rowIndex = op.startRow; rowIndex <= op.endRow; rowIndex += 1) {
                    const row = model.rows[rowIndex];
                    if (!row) continue;
                    const dataRow = op.data[rowIndex - op.startRow];
                    if (!dataRow) continue;
                    for (let colIndex = op.startCol; colIndex <= op.endCol; colIndex += 1) {
                        ensureRowWidth(row, colIndex);
                        row[colIndex] = dataRow[colIndex - op.startCol] ?? "";
                    }
                }
                break;
            }
        }
    }
}

function applyOps(ops: TableOp[]): void {
    applyOpsToModel(state, ops);
}

function postMirrorTextUpdate(requestId: number, userEvent: string): void {
    if (!state.liveMirrorEnabled) {
        return;
    }

    state.text = serializeCsv(state.headers, state.rows, state.delimiter);
    state.serializedVersion = state.version;

    const update: CsvMirrorTextUpdate = {
        text: state.text,
        userEvent,
        version: state.version,
    };

    postResponse({
        type: "mirror-text-update",
        requestId,
        update,
    });
}

function commitMutation(
    requestId: number,
    ops: TableOp[],
    pushToHistory: boolean,
    applied: boolean,
    mirrorUserEvent?: string,
): void {
    if (applied && ops.length > 0) {
        applyOps(ops);
        if (pushToHistory) {
            pushHistory(ops);
        }
        state.version += 1;

        if (state.liveMirrorEnabled && mirrorUserEvent) {
            postMirrorTextUpdate(requestId, mirrorUserEvent);
        } else {
            // Outside live-mirror sessions, keep the original source text only until
            // the first structural change. The latest text can be regenerated on the
            // final flush, which avoids holding a second full-document string in RAM
            // during large table-editing sessions.
            state.text = "";
            state.serializedVersion = DIRTY_SERIALIZED_VERSION;
        }
    }

    postResponse({
        type: "mutation-applied",
        requestId,
        snapshot: snapshot(),
        applied,
    });
}

function flushText(requestId: number): void {
    if (state.serializedVersion !== state.version) {
        state.text = serializeCsv(state.headers, state.rows, state.delimiter);
        state.serializedVersion = state.version;
    }

    postResponse({
        type: "flushed-text",
        requestId,
        text: state.text,
        version: state.version,
    });
}

function buildMutationOps(mutation: CsvMutationRequest): { ops: TableOp[]; applied: boolean } {
    switch (mutation.type) {
        case "edit-cell": {
            const row = state.rows[mutation.rowIndex];
            if (!row) return { ops: [], applied: false };
            const oldValue = row[mutation.colIndex] ?? "";
            if (oldValue === mutation.newValue) return { ops: [], applied: false };
            return {
                ops: [{ type: "cell", row: mutation.rowIndex, col: mutation.colIndex, oldValue, newValue: mutation.newValue }],
                applied: true,
            };
        }
        case "edit-header": {
            const oldValue = state.headers[mutation.colIndex] ?? "";
            if (oldValue === mutation.newValue) return { ops: [], applied: false };
            return {
                ops: [{ type: "header-cell", col: mutation.colIndex, oldValue, newValue: mutation.newValue }],
                applied: true,
            };
        }
        case "clear-cell": {
            const row = state.rows[mutation.rowIndex];
            if (!row) return { ops: [], applied: false };
            const oldValue = row[mutation.colIndex] ?? "";
            if (oldValue === "") return { ops: [], applied: false };
            return {
                ops: [{ type: "cell", row: mutation.rowIndex, col: mutation.colIndex, oldValue, newValue: "" }],
                applied: true,
            };
        }
        case "clear-selection": {
            const oldValues: string[][] = [];
            let hasChanges = false;
            for (let rowIndex = mutation.startRow; rowIndex <= mutation.endRow; rowIndex += 1) {
                const row = state.rows[rowIndex];
                const oldRow: string[] = [];
                for (let colIndex = mutation.startCol; colIndex <= mutation.endCol; colIndex += 1) {
                    const value = row?.[colIndex] ?? "";
                    oldRow.push(value);
                    if (value !== "") {
                        hasChanges = true;
                    }
                }
                oldValues.push(oldRow);
            }
            if (!hasChanges) return { ops: [], applied: false };
            return {
                ops: [{
                    type: "bulk-cell-clear",
                    startRow: mutation.startRow,
                    endRow: mutation.endRow,
                    startCol: mutation.startCol,
                    endCol: mutation.endCol,
                    oldValues,
                }],
                applied: true,
            };
        }
        case "delete-rows": {
            const deletedRows = cloneRows(state.rows.slice(mutation.start, mutation.end + 1));
            if (deletedRows.length === 0) return { ops: [], applied: false };
            return {
                ops: [{ type: "bulk-row-delete", start: mutation.start, end: mutation.end, data: deletedRows }],
                applied: true,
            };
        }
        case "delete-columns": {
            const deletedHeaders = state.headers.slice(mutation.start, mutation.end + 1);
            if (deletedHeaders.length === 0) return { ops: [], applied: false };
            const deletedData = state.rows.map((row) => row.slice(mutation.start, mutation.end + 1));
            return {
                ops: [{
                    type: "bulk-col-delete",
                    start: mutation.start,
                    end: mutation.end,
                    headers: deletedHeaders,
                    data: deletedData,
                }],
                applied: true,
            };
        }
        case "add-row": {
            const width = Math.max(state.headers.length, 1);
            return {
                ops: [{ type: "row-add", index: mutation.index, data: Array.from({ length: width }, () => "") }],
                applied: true,
            };
        }
        case "add-column": {
            const rowCount = Math.max(state.rows.length, 1);
            return {
                ops: [{
                    type: "bulk-col-add",
                    start: mutation.index,
                    headers: [""],
                    data: Array.from({ length: rowCount }, () => [""]),
                }],
                applied: true,
            };
        }
        case "move-rows": {
            const count = mutation.end - mutation.start + 1;
            const targetStart = mutation.start + mutation.direction;
            if (targetStart < 0 || targetStart + count > state.rows.length) {
                return { ops: [], applied: false };
            }
            const movedRows = cloneRows(state.rows.slice(mutation.start, mutation.end + 1));
            return {
                ops: [
                    {
                        type: "bulk-row-delete",
                        start: mutation.start,
                        end: mutation.end,
                        data: movedRows,
                    },
                    {
                        type: "bulk-row-add",
                        start: targetStart,
                        data: cloneRows(movedRows),
                    },
                ],
                applied: true,
            };
        }
        case "move-columns": {
            const count = mutation.end - mutation.start + 1;
            const targetStart = mutation.start + mutation.direction;
            if (targetStart < 0 || targetStart + count > state.headers.length) {
                return { ops: [], applied: false };
            }
            const movedHeaders = state.headers.slice(mutation.start, mutation.end + 1);
            if (movedHeaders.length === 0) {
                return { ops: [], applied: false };
            }
            const movedData = state.rows.map((row) => row.slice(mutation.start, mutation.end + 1));
            return {
                ops: [
                    {
                        type: "bulk-col-delete",
                        start: mutation.start,
                        end: mutation.end,
                        headers: [...movedHeaders],
                        data: cloneRows(movedData),
                    },
                    {
                        type: "bulk-col-add",
                        start: targetStart,
                        headers: [...movedHeaders],
                        data: cloneRows(movedData),
                    },
                ],
                applied: true,
            };
        }
    }
}

function parseAndInitialize(text: string, requestId: number): void {
    state.headers = [];
    state.rows = [];
    state.delimiter = ",";
    state.errors = [];
    state.text = text;
    state.undoStack = [];
    state.redoStack = [];
    state.liveMirrorEnabled = false;
    state.version += 1;
    state.serializedVersion = state.version;

    if (!text.trim()) {
        state.liveMirrorEnabled = true;
        postResponse({ type: "initialized", requestId, snapshot: snapshot() });
        return;
    }

    let isFirstRow = true;
    let parsedRows = 0;

    Papa.parse<string[]>(text, {
        header: false,
        skipEmptyLines: "greedy",
        delimitersToGuess: [",", "\t", ";", "|", ":", "~"],
        step(results) {
            for (const error of results.errors) {
                state.errors.push(`Row ${error.row}: ${error.message}`);
            }

            if (isFirstRow) {
                state.headers = [...results.data];
                state.delimiter = results.meta.delimiter ?? ",";
                isFirstRow = false;
                return;
            }

            state.rows.push([...results.data]);
            parsedRows += 1;

            if (parsedRows % CHUNK_SIZE === 0) {
                postResponse({
                    type: "initialize-progress",
                    requestId,
                    parsedRows,
                });
            }
        },
        complete() {
            state.liveMirrorEnabled = state.rows.length <= LIVE_MIRROR_ROW_THRESHOLD;
            postResponse({
                type: "initialized",
                requestId,
                snapshot: snapshot(),
            });
        },
        error(error: Error) {
            postResponse({
                type: "error",
                requestId,
                error: error.message,
            });
        },
    });
}

self.onmessage = (event: MessageEvent<CsvWorkerRequest>) => {
    const message = event.data;

    try {
        switch (message.type) {
            case "initialize":
                parseAndInitialize(message.text, message.requestId);
                return;
            case "get-rows": {
                const start = Math.max(0, message.start);
                const end = Math.min(state.rows.length - 1, message.end);
                postResponse({
                    type: "rows",
                    requestId: message.requestId,
                    window: {
                        start,
                        // postMessage already structured-clones the payload crossing the
                        // worker boundary, so avoid an extra deep clone here.
                        rows: start <= end ? state.rows.slice(start, end + 1) : [],
                        version: state.version,
                    },
                });
                return;
            }
            case "get-cell": {
                if (message.rowIndex === -1) {
                    postResponse({
                        type: "cell",
                        requestId: message.requestId,
                        value: state.headers[message.colIndex] ?? "",
                    });
                    return;
                }

                postResponse({
                    type: "cell",
                    requestId: message.requestId,
                    value: state.rows[message.rowIndex]?.[message.colIndex] ?? "",
                });
                return;
            }
            case "mutate": {
                const { ops, applied } = buildMutationOps(message.mutation);
                commitMutation(message.requestId, ops, true, applied, message.userEvent);
                return;
            }
            case "undo": {
                const entry = state.undoStack.pop();
                if (!entry) {
                    commitMutation(message.requestId, [], false, false);
                    return;
                }
                state.redoStack.push(entry);
                commitMutation(message.requestId, invertOps(entry), false, true, "undo.table");
                return;
            }
            case "redo": {
                const entry = state.redoStack.pop();
                if (!entry) {
                    commitMutation(message.requestId, [], false, false);
                    return;
                }
                state.undoStack.push(entry);
                commitMutation(message.requestId, entry, false, true, "redo.table");
                return;
            }
            case "flush-text": {
                flushText(message.requestId);
                return;
            }
        }
    } catch (error) {
        postResponse({
            type: "error",
            requestId: message.requestId,
            error: error instanceof Error ? error.message : "CSV worker error",
        });
    }
};