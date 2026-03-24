use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// Maximum number of preview lines stored per file during content scanning.
pub const MAX_PREVIEWS_PER_FILE: usize = 50;

/// Maximum character length for a single preview line text. Lines longer
/// than this are truncated to keep the IPC payload bounded.
const MAX_PREVIEW_LINE_LENGTH: usize = 500;

/// Characters to preserve before the first match in a line excerpt.
const LINE_EXCERPT_CONTEXT_BEFORE: usize = 20;
/// Maximum total length of a line excerpt.
const LINE_EXCERPT_MAX_LENGTH: usize = 80;

/// A text fragment with a flag indicating whether it matched a search term.
/// Sent to the frontend so it can render highlights without any regex work.
#[derive(Clone, Serialize)]
pub struct HighlightFragment {
    pub text: String,
    pub is_match: bool,
}

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
    pub fragments: Vec<HighlightFragment>,
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
    /// Pre-computed highlight fragments for the file name.
    pub filename_fragments: Vec<HighlightFragment>,
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

/// Returns a short excerpt of `line_text` centred around the first query-term
/// match. Leading/trailing truncation is indicated with "…".
pub fn get_line_excerpt(line_text: &str, terms: &[String]) -> String {
    let trimmed = line_text.trim();
    if trimmed.len() <= LINE_EXCERPT_MAX_LENGTH || terms.is_empty() {
        if trimmed.len() > LINE_EXCERPT_MAX_LENGTH {
            let end = floor_char_boundary(trimmed, LINE_EXCERPT_MAX_LENGTH);
            return format!("{}…", &trimmed[..end]);
        }
        return trimmed.to_string();
    }

    let lower = trimmed.to_lowercase();
    let mut match_start: Option<usize> = None;
    for term in terms {
        if term.is_empty() {
            continue;
        }
        if let Some(idx) = lower.find(term.as_str()) {
            match_start = Some(match match_start {
                Some(prev) if prev <= idx => prev,
                _ => idx,
            });
        }
    }

    let anchor = match match_start {
        Some(pos) => pos,
        None => {
            let end = floor_char_boundary(trimmed, LINE_EXCERPT_MAX_LENGTH);
            return format!("{}…", &trimmed[..end]);
        }
    };

    let raw_start = anchor.saturating_sub(LINE_EXCERPT_CONTEXT_BEFORE);
    let start = ceil_char_boundary(trimmed, raw_start);
    let raw_end = (start + LINE_EXCERPT_MAX_LENGTH).min(trimmed.len());
    let end = floor_char_boundary(trimmed, raw_end);

    let mut excerpt = String::new();
    if start > 0 {
        excerpt.push('…');
    }
    excerpt.push_str(&trimmed[start..end]);
    if end < trimmed.len() {
        excerpt.push('…');
    }
    excerpt
}

/// Splits `text` into alternating match / non-match fragments against the
/// given `terms` (case-insensitive). Terms are tried longest-first so that
/// a longer term is preferred over a shorter prefix.
pub fn split_text_by_terms(text: &str, terms: &[String]) -> Vec<HighlightFragment> {
    if text.is_empty() || terms.is_empty() {
        return vec![HighlightFragment {
            text: text.to_string(),
            is_match: false,
        }];
    }

    // Sort terms longest-first so longer matches take priority.
    let mut sorted_terms: Vec<&str> = terms.iter().map(|t| t.as_str()).collect();
    sorted_terms.sort_by(|a, b| b.len().cmp(&a.len()));

    let lower_text = text.to_lowercase();
    let mut fragments: Vec<HighlightFragment> = Vec::new();
    let mut cursor = 0;

    while cursor < text.len() {
        let remaining = &lower_text[cursor..];
        let mut best_match: Option<(usize, usize)> = None; // (offset, length)

        for term in &sorted_terms {
            if term.is_empty() {
                continue;
            }
            if let Some(offset) = remaining.find(term.as_ref() as &str) {
                match best_match {
                    Some((prev_offset, prev_len))
                        if offset < prev_offset
                            || (offset == prev_offset && term.len() > prev_len) =>
                    {
                        best_match = Some((offset, term.len()));
                    }
                    None => {
                        best_match = Some((offset, term.len()));
                    }
                    _ => {}
                }
            }
        }

        match best_match {
            Some((offset, len)) => {
                let abs_start = cursor + offset;
                if abs_start > cursor {
                    fragments.push(HighlightFragment {
                        text: text[cursor..abs_start].to_string(),
                        is_match: false,
                    });
                }
                fragments.push(HighlightFragment {
                    text: text[abs_start..abs_start + len].to_string(),
                    is_match: true,
                });
                cursor = abs_start + len;
            }
            None => {
                fragments.push(HighlightFragment {
                    text: text[cursor..].to_string(),
                    is_match: false,
                });
                break;
            }
        }
    }

    if fragments.is_empty() {
        return vec![HighlightFragment {
            text: text.to_string(),
            is_match: false,
        }];
    }

    fragments
}

/// Finds the largest index ≤ `index` that is a char boundary.
fn floor_char_boundary(s: &str, mut index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    while index > 0 && !s.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// Finds the smallest index ≥ `index` that is a char boundary.
fn ceil_char_boundary(s: &str, mut index: usize) -> usize {
    while index < s.len() && !s.is_char_boundary(index) {
        index += 1;
    }
    index
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
