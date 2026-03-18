use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// Maximum number of preview lines stored per file during content scanning.
pub const MAX_PREVIEWS_PER_FILE: usize = 50;

/// Maximum character length for a single preview line text. Lines longer
/// than this are truncated to keep the IPC payload bounded.
const MAX_PREVIEW_LINE_LENGTH: usize = 500;

#[derive(Clone, Default)]
pub struct SearchPreview {
    pub line_number: Option<u64>,
    pub line_text: String,
}

#[derive(Clone, Default)]
pub struct ContentMatchSummary {
    pub total_hits: usize,
    pub term_frequencies: HashMap<String, usize>,
    pub document_frequencies: HashMap<String, usize>,
    pub previews: Vec<SearchPreview>,
}

#[derive(Clone, Default)]
pub struct FileSearchCandidate {
    pub path: String,
    pub file_name: String,
    pub extension: Option<String>,
    pub source: String,
    pub exists_on_disk: bool,
    pub size_bytes: Option<u64>,
    pub last_opened_at: Option<i64>,
    pub last_saved_at: Option<i64>,
    pub last_seen_at: Option<i64>,
    pub last_modified_at: Option<i64>,
    pub pinned: bool,
    pub content: ContentMatchSummary,
    pub document_length: f32,
}

/// A single matched line returned to the frontend for display.
#[derive(Clone, Serialize)]
pub struct MatchedLine {
    pub line_number: u64,
    pub line_text: String,
}

#[derive(Clone, Serialize)]
pub struct SearchResultRecord {
    pub path: String,
    pub file_name: String,
    pub extension: Option<String>,
    pub source: String,
    pub exists_on_disk: bool,
    pub size_bytes: Option<u64>,
    pub last_opened_at: Option<i64>,
    pub last_saved_at: Option<i64>,
    pub last_seen_at: Option<i64>,
    pub last_modified_at: Option<i64>,
    pub pinned: bool,
    pub matched_lines: Vec<MatchedLine>,
    pub match_count: usize,
    pub filename_score: f32,
    pub content_score: f32,
    pub freshness_score: f32,
    pub usage_score: f32,
    pub final_score: f32,
}

/// Truncates a preview line to `MAX_PREVIEW_LINE_LENGTH`, splitting at a
/// char boundary so the result is always valid UTF-8.
pub fn truncate_preview_line(text: &str) -> String {
    let trimmed = text.trim_end();
    if trimmed.len() <= MAX_PREVIEW_LINE_LENGTH {
        return trimmed.to_string();
    }
    let mut end = MAX_PREVIEW_LINE_LENGTH;
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &trimmed[..end])
}

pub fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

pub fn system_time_to_unix_ms(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as i64)
}
