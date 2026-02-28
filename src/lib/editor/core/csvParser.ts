import Papa from "papaparse";

export interface CsvParseResult {
    headers: string[];
    rows: string[][];
    delimiter: string;
    errors: string[];
}

/**
 * Parse CSV text with auto-detected delimiter.
 * Supports comma, tab, semicolon, pipe, and other standard delimiters.
 */
export function parseCsv(text: string): CsvParseResult {
    if (!text.trim()) {
        return { headers: [], rows: [], delimiter: ",", errors: [] };
    }

    const result = Papa.parse<string[]>(text, {
        header: false,
        skipEmptyLines: "greedy",
        delimitersToGuess: [",", "\t", ";", "|", ":", "~"],
    });

    const data = result.data;
    const delimiter = result.meta.delimiter;
    const errors = result.errors.map(
        (e) => `Row ${e.row}: ${e.message}`,
    );

    if (data.length === 0) {
        return { headers: [], rows: [], delimiter, errors };
    }

    // First row is headers
    const headers = data[0];
    const rows = data.slice(1);

    return { headers, rows, delimiter, errors };
}

/**
 * Serialize parsed CSV data back to a string.
 */
export function serializeCsv(
    headers: string[],
    rows: string[][],
    delimiter: string,
): string {
    const allData = [headers, ...rows];
    return Papa.unparse(allData, {
        delimiter,
        newline: "\n",
    });
}
