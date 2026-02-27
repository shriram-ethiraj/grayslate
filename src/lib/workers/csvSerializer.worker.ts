import Papa from "papaparse";
import type { TableOp } from "../components/editor/csv/useCsvHistory.svelte";

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
            applyOpsLocally(msg.ops, msg.reverse);
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

function applyOpsLocally(ops: TableOp[], reverse: boolean) {
    const opsArray = reverse ? [...ops].reverse() : ops;

    for (const op of opsArray) {
        switch (op.type) {
            case 'cell': {
                const rowArr = internalRows[op.row];
                if (rowArr) {
                    while (rowArr.length <= op.col) rowArr.push("");
                    rowArr[op.col] = reverse ? op.oldValue : op.newValue;
                }
                break;
            }
            case 'header-cell': {
                internalHeaders[op.col] = reverse ? op.oldValue : op.newValue;
                break;
            }
            case 'row-add':
                if (reverse) {
                    internalRows.splice(op.index, 1);
                } else {
                    internalRows.splice(op.index, 0, [...op.data]);
                }
                break;
            case 'row-delete':
                if (reverse) {
                    internalRows.splice(op.index, 0, [...op.data]);
                } else {
                    internalRows.splice(op.index, 1);
                }
                break;
        }
    }
}
