use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

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
    pub preview: Option<SearchPreview>,
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
    pub preview_line: Option<String>,
    pub preview_line_number: Option<u64>,
    pub match_count: usize,
    pub filename_score: f32,
    pub content_score: f32,
    pub freshness_score: f32,
    pub usage_score: f32,
    pub final_score: f32,
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