/**
 * csvUtils.ts
 *
 * Shared CSV parsing utilities used by both the rainbow-highlight
 * decoration plugin and the CSV sticky-scroll header.
 *
 * Kept in its own module so the two extensions can share code without
 * creating a circular import.
 */

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Number of distinct rainbow colours in the cycling palette. */
export const NUM_COLORS = 10;

// ---------------------------------------------------------------------------
// Delimiter detection
// ---------------------------------------------------------------------------

const CANDIDATE_DELIMITERS = [",", "\t", ";", "|"] as const;

/**
 * Heuristically determine the field delimiter by counting occurrences of each
 * candidate in the first `sampleSize` characters of the document.
 * Returns the delimiter with the highest occurrence count; falls back to `,`.
 */
export function detectDelimiter(sample: string): string {
    let best = ",";
    let bestCount = -1;
    for (const d of CANDIDATE_DELIMITERS) {
        let count = 0;
        let from = 0;
        while ((from = sample.indexOf(d, from)) !== -1) {
            count++;
            from += d.length;
        }
        if (count > bestCount) {
            bestCount = count;
            best = d;
        }
    }
    return best;
}

// ---------------------------------------------------------------------------
// RFC-4180 field-range extractor
// ---------------------------------------------------------------------------

export interface FieldRange {
    /** Inclusive start within the line string. */
    from: number;
    /** Exclusive end within the line string (character *after* last field char). */
    to: number;
}

/**
 * Parse a single CSV line and return the character ranges of each field.
 *
 * Handles:
 *  - Unquoted fields separated by `delimiter`
 *  - Double-quoted fields (RFC 4180), including embedded delimiters and
 *    doubled-quote escape sequences `""`
 *
 * @param line      Raw line text (no trailing newline)
 * @param delimiter Field separator string (typically one char)
 */
export function getFieldRanges(line: string, delimiter: string): FieldRange[] {
    const ranges: FieldRange[] = [];
    const dlen = delimiter.length;
    let i = 0;

    while (i <= line.length) {
        const fieldStart = i;

        if (i < line.length && line[i] === '"') {
            // ── Quoted field ──────────────────────────────────────────────
            let j = i + 1;
            while (j < line.length) {
                if (line[j] === '"') {
                    if (line[j + 1] === '"') {
                        // doubled-quote escape: skip both
                        j += 2;
                    } else {
                        // closing quote
                        j++;
                        break;
                    }
                } else {
                    j++;
                }
            }
            // j is now one past the closing quote (or at line.length if
            // the field was never properly closed — be tolerant)
            ranges.push({ from: fieldStart, to: j });

            if (j < line.length && line.slice(j, j + dlen) === delimiter) {
                i = j + dlen;
            } else {
                // end of line (or malformed — stop gracefully)
                break;
            }
        } else {
            // ── Unquoted field ────────────────────────────────────────────
            const delimIdx = line.indexOf(delimiter, i);
            if (delimIdx === -1) {
                ranges.push({ from: fieldStart, to: line.length });
                break;
            } else {
                ranges.push({ from: fieldStart, to: delimIdx });
                i = delimIdx + dlen;
            }
        }
    }

    return ranges;
}

// ---------------------------------------------------------------------------
// Field value decoder
// ---------------------------------------------------------------------------

/**
 * Decode a raw CSV field value as returned by {@link getFieldRanges}.
 *
 * RFC 4180 decoding rules:
 *  - Quoted field (`"foo"`)   — strip outer double-quotes, unescape `""` → `"`
 *  - Unquoted field (`foo`)   — return the value unchanged
 *
 * This is intentionally minimal: it covers the tooltip / display case
 * (showing a field's *logical* content rather than its raw stored form)
 * without replacing the full Rust-side CSV deserialiser.
 */
export function decodeFieldValue(raw: string): string {
    if (raw.length >= 2 && raw[0] === '"' && raw[raw.length - 1] === '"') {
        return raw.slice(1, -1).replace(/""/g, '"');
    }
    return raw;
}
