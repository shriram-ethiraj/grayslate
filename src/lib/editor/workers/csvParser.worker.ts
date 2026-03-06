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

    (Papa.parse as any)(text, {
        header: false,
        skipEmptyLines: "greedy",
        delimitersToGuess: [",", "\t", ";", "|", ":", "~"],
        step: function (results: any) {
            if (isFirstRow) {
                headers = results.data as string[];
                detectedDelimiter = results.meta.delimiter;
                isFirstRow = false;
                return;
            }

            currentChunk.push(results.data as string[]);
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
        error: function (error: Papa.ParseError) {
            errors.push(`Row ${error.row}: ${error.message}`);
        },
        complete: function () {
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
