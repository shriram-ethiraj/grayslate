import Papa from "papaparse";

self.onmessage = (e: MessageEvent) => {
    const { headers, rows, delimiter } = e.data;

    try {
        const allData = [headers, ...rows];
        const serialized = Papa.unparse(allData, {
            delimiter,
            newline: "\n",
        });

        self.postMessage({ serialized });
    } catch (error) {
        self.postMessage({ error: (error as Error).message });
    }
};
