use std::sync::atomic::{AtomicBool, Ordering};

use super::session::CsvSession;

/// CSV delimiter candidates, matching PapaParse's `delimitersToGuess` order.
const DELIMITER_CANDIDATES: &[u8] = &[b',', b'\t', b';', b'|', b':', b'~'];

// ---------------------------------------------------------------------------
// Delimiter detection
// ---------------------------------------------------------------------------

/// Detect the most likely CSV delimiter from the first line. Quote-aware:
/// delimiters inside `"..."` blocks are ignored. Falls back to comma.
pub fn detect_delimiter(text: &str) -> u8 {
    let first_line = text.lines().next().unwrap_or("").as_bytes();
    let mut counts = [0u32; 6];
    let mut in_quote = false;

    for &byte in first_line {
        if byte == b'"' {
            in_quote = !in_quote;
            continue;
        }
        if in_quote {
            continue;
        }
        for (i, &candidate) in DELIMITER_CANDIDATES.iter().enumerate() {
            if byte == candidate {
                counts[i] += 1;
            }
        }
    }

    counts
        .iter()
        .enumerate()
        .filter(|(_, &c)| c > 0)
        .max_by_key(|(_, &c)| c)
        .map(|(i, _)| DELIMITER_CANDIDATES[i])
        .unwrap_or(b',')
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Row count reporting interval during streaming parse.
const PROGRESS_CHUNK: usize = 50_000;

/// Parse CSV text into a `CsvSession`, with optional progress reporting and
/// cancellation. Returns the fully initialized session.
pub fn parse_csv<F>(
    text: &str,
    cancelled: &AtomicBool,
    progress_fn: F,
) -> Result<CsvSession, String>
where
    F: Fn(usize),
{
    let mut session = CsvSession::empty();
    session.set_serialized_text(text.to_string());

    let trimmed = text.trim();
    if trimmed.is_empty() {
        session.live_mirror_enabled = true;
        return Ok(session);
    }

    let delimiter = detect_delimiter(trimmed);
    session.delimiter = delimiter;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(trimmed.as_bytes());

    let mut record = csv::StringRecord::new();
    let mut is_first_row = true;
    let mut parsed_rows: usize = 0;

    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err("CSV parsing cancelled.".to_string());
        }

        match rdr.read_record(&mut record) {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) => {
                session.errors.push(format!("Row {}: {}", parsed_rows, e));
                continue;
            }
        }

        // Skip fully empty records (mirrors PapaParse's skipEmptyLines: "greedy").
        if record.iter().all(|f| f.trim().is_empty()) {
            continue;
        }

        if is_first_row {
            session.headers = record.iter().map(|f| f.to_string()).collect();
            is_first_row = false;
            continue;
        }

        session
            .rows
            .push(record.iter().map(|f| f.to_string()).collect());
        parsed_rows += 1;

        if parsed_rows % PROGRESS_CHUNK == 0 {
            progress_fn(parsed_rows);
        }
    }

    session.finalize_after_parse();
    Ok(session)
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

/// Serialize headers + rows back to CSV text using the given delimiter.
/// Matches the JS `serializeCsv` output: LF line endings, fields quoted only
/// when necessary. No trailing newline.
pub fn serialize_csv(headers: &[String], rows: &[Vec<String>], delimiter: u8) -> String {
    let col_count = headers.len();
    let estimated = (rows.len() + 1) * col_count * 20;
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::with_capacity(estimated));

    let _ = wtr.write_record(headers);

    for row in rows {
        let _ = wtr.write_record(row);
    }

    let bytes = wtr.into_inner().unwrap_or_default();
    let mut text = String::from_utf8(bytes).unwrap_or_default();
    // Strip trailing newline to match PapaParse's unparse output.
    if text.ends_with('\n') {
        text.pop();
    }
    text
}
