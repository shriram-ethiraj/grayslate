use super::mutation::{build_mutation_ops, CsvMutationRequest, MutationResult};
use super::ops::{apply_ops, invert_ops, TableModel, TableOp};
use super::parser::serialize_csv;
use super::types::{CsvMutationResponse, CsvRowWindow, CsvTableSnapshot};

/// Maximum undo/redo history entries.
const MAX_HISTORY: usize = 200;

/// Row count at or below which live mirroring to CodeMirror is enabled.
pub(crate) const LIVE_MIRROR_ROW_THRESHOLD: usize = 100_000;

/// Sentinel value indicating the serialized text cache is stale.
const DIRTY_SERIALIZED_VERSION: i64 = -1;

// ---------------------------------------------------------------------------
// CsvSession — the full per-window CSV table state
// ---------------------------------------------------------------------------

pub struct CsvSession {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub delimiter: u8,
    pub errors: Vec<String>,
    pub version: u64,
    serialized_version: i64,
    serialized_text: String,
    undo_stack: Vec<Vec<TableOp>>,
    redo_stack: Vec<Vec<TableOp>>,
    pub live_mirror_enabled: bool,
}

impl CsvSession {
    /// Create a blank session. Used by `parse_csv` which populates fields
    /// incrementally during streaming parse.
    pub(crate) fn empty() -> Self {
        CsvSession {
            headers: Vec::new(),
            rows: Vec::new(),
            delimiter: b',',
            errors: Vec::new(),
            version: 1,
            serialized_version: 1,
            serialized_text: String::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            live_mirror_enabled: false,
        }
    }

    /// Store the raw text that was used to parse this session. Called by
    /// `parse_csv` before any rows are read.
    pub(crate) fn set_serialized_text(&mut self, text: String) {
        self.serialized_text = text;
    }

    /// Finalize after parse: set `live_mirror_enabled` based on row count.
    pub(crate) fn finalize_after_parse(&mut self) {
        self.live_mirror_enabled = self.rows.len() <= LIVE_MIRROR_ROW_THRESHOLD;
    }

    // -- Snapshot -----------------------------------------------------------

    pub fn snapshot(&self) -> CsvTableSnapshot {
        CsvTableSnapshot {
            headers: self.headers.clone(),
            row_count: self.rows.len(),
            delimiter: String::from(self.delimiter as char),
            errors: self.errors.clone(),
            version: self.version,
            live_mirror_enabled: self.live_mirror_enabled,
        }
    }

    // -- Read operations ----------------------------------------------------

    pub fn get_rows(&self, start: usize, end: usize) -> CsvRowWindow {
        let s = start.min(self.rows.len());
        let e = (end + 1).min(self.rows.len());
        let rows = if s < e {
            self.rows[s..e].to_vec()
        } else {
            Vec::new()
        };
        CsvRowWindow {
            start: s,
            rows,
            version: self.version,
        }
    }

    pub fn get_cell(&self, row_index: i64, col_index: usize) -> String {
        if row_index == -1 {
            self.headers.get(col_index).cloned().unwrap_or_default()
        } else {
            let ri = row_index as usize;
            self.rows
                .get(ri)
                .and_then(|r| r.get(col_index))
                .cloned()
                .unwrap_or_default()
        }
    }

    // -- Mutation building --------------------------------------------------

    pub fn build_mutation_ops(&self, mutation: &CsvMutationRequest) -> MutationResult {
        build_mutation_ops(&self.headers, &self.rows, mutation)
    }

    // -- Commit + history ---------------------------------------------------

    fn push_history(&mut self, ops: Vec<TableOp>) {
        if ops.is_empty() {
            return;
        }
        self.undo_stack.push(ops);
        if self.undo_stack.len() > MAX_HISTORY {
            let excess = self.undo_stack.len() - MAX_HISTORY;
            self.undo_stack.drain(..excess);
        }
        self.redo_stack.clear();
    }

    pub fn commit_mutation(
        &mut self,
        ops: Vec<TableOp>,
        push_to_history: bool,
        applied: bool,
        mirror_user_event: Option<&str>,
    ) -> CsvMutationResponse {
        let mut mirror_text = None;
        let mut mirror_event = None;

        if applied && !ops.is_empty() {
            {
                let mut model = TableModel {
                    headers: &mut self.headers,
                    rows: &mut self.rows,
                };
                apply_ops(&mut model, &ops);
            }
            if push_to_history {
                self.push_history(ops);
            }
            self.version += 1;

            if self.live_mirror_enabled {
                if let Some(event) = mirror_user_event {
                    let text = serialize_csv(&self.headers, &self.rows, self.delimiter);
                    self.serialized_text = text.clone();
                    self.serialized_version = self.version as i64;
                    mirror_text = Some(text);
                    mirror_event = Some(event.to_string());
                }
            } else {
                // Drop cached text to free memory (regenerated on flush).
                self.serialized_text = String::new();
                self.serialized_version = DIRTY_SERIALIZED_VERSION;
            }
        }

        CsvMutationResponse {
            snapshot: self.snapshot(),
            applied,
            mirror_text,
            mirror_user_event: mirror_event,
        }
    }

    pub fn mutate(
        &mut self,
        mutation: &CsvMutationRequest,
        user_event: &str,
    ) -> CsvMutationResponse {
        let MutationResult { ops, applied } = self.build_mutation_ops(mutation);
        self.commit_mutation(ops, true, applied, Some(user_event))
    }

    pub fn undo(&mut self) -> CsvMutationResponse {
        let entry = self.undo_stack.pop();
        match entry {
            Some(ops) => {
                let inverted = invert_ops(&ops);
                self.redo_stack.push(ops);
                self.commit_mutation(inverted, false, true, Some("undo.table"))
            }
            None => CsvMutationResponse {
                snapshot: self.snapshot(),
                applied: false,
                mirror_text: None,
                mirror_user_event: None,
            },
        }
    }

    pub fn redo(&mut self) -> CsvMutationResponse {
        let entry = self.redo_stack.pop();
        match entry {
            Some(ops) => {
                self.undo_stack.push(ops.clone());
                self.commit_mutation(ops, false, true, Some("redo.table"))
            }
            None => CsvMutationResponse {
                snapshot: self.snapshot(),
                applied: false,
                mirror_text: None,
                mirror_user_event: None,
            },
        }
    }

    // -- Flush (serialization) ----------------------------------------------

    pub fn flush_text(&mut self) -> String {
        if self.serialized_version != self.version as i64 {
            self.serialized_text =
                serialize_csv(&self.headers, &self.rows, self.delimiter);
            self.serialized_version = self.version as i64;
        }
        self.serialized_text.clone()
    }

    pub fn flush_version(&self) -> u64 {
        self.version
    }

    /// Test helper: check if the serialized text cache is empty.
    #[cfg(test)]
    pub fn serialized_text_is_empty(&self) -> bool {
        self.serialized_text.is_empty()
    }
}
