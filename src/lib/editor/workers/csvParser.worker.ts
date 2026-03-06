import Papa from "papaparse";

const CHUNK_SIZE = 50_000;

self.onmessage = (e: MessageEvent) => {
    const requestId: number = e.data.requestId ?? 0;
    const text: string = e.data.text;

    if (!text.trim()) {
        self.postMessage({
            type: "parsed-complete",
            requestId,
            headers: [],
            delimiter: ",",
            errors: [],
            totalRows: 0,
        });
        return;
    }

    let headers: string[] = [];
    let isFirstRow = true;
    let detectedDelimiter = ",";
    const errors: string[] = [];
    let currentChunk: string[][] = [];
    let offset = 0;
    let totalRows = 0;

    Papa.parse<string[]>(text, {
        header: false,
        skipEmptyLines: "greedy",
        delimitersToGuess: [",", "\t", ";", "|", ":", "~"],
        step(results: Papa.ParseStepResult<string[]>) {
            for (const error of results.errors) {
                errors.push(`Row ${error.row}: ${error.message}`);
            }

            if (isFirstRow) {
                headers = results.data;
                detectedDelimiter = results.meta.delimiter ?? ",";
                isFirstRow = false;
                return;
            }

            currentChunk.push(results.data);
            totalRows++;

            if (currentChunk.length >= CHUNK_SIZE) {
                self.postMessage({
                    type: "parsed-chunk",
                    requestId,
                    chunk: currentChunk,
                    offset,
                });
                offset += currentChunk.length;
                currentChunk = [];
            }
        },
        complete() {
            // Send remaining chunk
            if (currentChunk.length > 0) {
                self.postMessage({
                    type: "parsed-chunk",
                    requestId,
                    chunk: currentChunk,
                    offset,
                });
            }

            // Final message with metadata
            self.postMessage({
                type: "parsed-complete",
                requestId,
                headers,
                delimiter: detectedDelimiter,
                errors,
                totalRows,
            });
        },
    });
};
