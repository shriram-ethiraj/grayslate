use serde::Serialize;

/// Snapshot returned to the frontend after init, mutation, undo, redo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvTableSnapshot {
    pub headers: Vec<String>,
    pub row_count: usize,
    pub delimiter: String,
    pub errors: Vec<String>,
    pub version: u64,
    pub live_mirror_enabled: bool,
}

/// Row window returned for viewport rendering.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvRowWindow {
    pub start: usize,
    pub rows: Vec<Vec<String>>,
    pub version: u64,
}

/// Result of a mutation / undo / redo operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvMutationResponse {
    pub snapshot: CsvTableSnapshot,
    pub applied: bool,
    /// Serialized CSV text for live-mirror sessions. `None` for large sessions.
    pub mirror_text: Option<String>,
    /// The userEvent label for the mirror update, if applicable.
    pub mirror_user_event: Option<String>,
}
