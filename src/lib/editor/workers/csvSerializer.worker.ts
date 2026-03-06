import Papa from "papaparse";
import type { WorkerOp } from "$lib/editor/components/csv/useCsvHistory.svelte";

let internalHeaders: string[] = [];
let internalRows: string[][] = [];
let internalDelimiter = ",";

self.onmessage = (e: MessageEvent) => {
    const msg = e.data;

    switch (msg.type) {
        case 'INIT_START':
            internalHeaders = [];
            internalRows = [];
            internalDelimiter = ",";
            break;
        case 'INIT_CHUNK':
            for (let i = 0; i < msg.chunk.length; i++) {
                internalRows.push(msg.chunk[i]);
            }
            break;
        case 'INIT_DONE':
            internalHeaders = msg.headers;
            internalDelimiter = msg.delimiter;
            break;
        case 'UPDATE_OPS':
            applyOpsLocally(msg.ops);
            break;
        case 'SERIALIZE':
            try {
                const allData = [internalHeaders, ...internalRows];
                const serialized = Papa.unparse(allData, {
                    delimiter: internalDelimiter,
                    newline: "\n",
                });
                self.postMessage({ serialized });
            } catch (error) {
                self.postMessage({ error: (error as Error).message });
            }
            break;
        default:
            // Fallback for legacy calls if any
            if (msg.headers && msg.rows) {
                try {
                    const allData = [msg.headers, ...msg.rows];
                    const serialized = Papa.unparse(allData, {
                        delimiter: msg.delimiter || ",",
                        newline: "\n",
                    });
                    self.postMessage({ serialized });
                } catch (error) {
                    self.postMessage({ error: (error as Error).message });
                }
            }
            break;
    }
};

function applyOpsLocally(ops: WorkerOp[]) {
    for (const op of ops) {
        switch (op.type) {
            case 'cell': {
                const rowArr = internalRows[op.row];
                if (rowArr) {
                    while (rowArr.length <= op.col) rowArr.push("");
                    rowArr[op.col] = op.value;
                }
                break;
            }
            case 'header-cell': {
                internalHeaders[op.col] = op.value;
                break;
            }
            case 'row-add': {
                internalRows = [
                    ...internalRows.slice(0, op.index),
                    ...op.data,
                    ...internalRows.slice(op.index)
                ];
                break;
            }
            case 'row-delete': {
                internalRows.splice(op.index, op.count);
                break;
            }
            case 'col-add': {
                internalHeaders = [
                    ...internalHeaders.slice(0, op.index),
                    ...op.headers,
                    ...internalHeaders.slice(op.index)
                ];
                for (let i = 0; i < internalRows.length; i++) {
                    const dataRow = op.data[i] || [];
                    internalRows[i] = [
                        ...internalRows[i].slice(0, op.index),
                        ...dataRow,
                        ...internalRows[i].slice(op.index)
                    ];
                }
                break;
            }
            case 'col-delete': {
                internalHeaders.splice(op.index, op.count);
                for (let i = 0; i < internalRows.length; i++) {
                    internalRows[i].splice(op.index, op.count);
                }
                break;
            }
            case 'cell-clear': {
                for (let r = op.startRow; r <= op.endRow; r++) {
                    if (r >= 0 && r < internalRows.length) {
                        for (let c = op.startCol; c <= op.endCol; c++) {
                            internalRows[r][c] = "";
                        }
                    }
                }
                break;
            }
            case 'cell-fill': {
                for (let r = op.startRow; r <= op.endRow; r++) {
                    if (r >= 0 && r < internalRows.length) {
                        const dataRowIndex = r - op.startRow;
                        const dataRow = op.data[dataRowIndex];
                        if (dataRow) {
                            for (let c = op.startCol; c <= op.endCol; c++) {
                                const dataColIndex = c - op.startCol;
                                internalRows[r][c] = dataRow[dataColIndex] !== undefined ? dataRow[dataColIndex] : "";
                            }
                        }
                    }
                }
                break;
            }
        }
    }
}
