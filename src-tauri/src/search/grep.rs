use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use grep_regex::RegexMatcherBuilder;
use grep_searcher::{SearcherBuilder, Sink, SinkMatch};
use ignore::WalkBuilder;

use crate::storage::normalize_path_key;

use super::{
    query::ParsedSearchQuery,
    scope::SearchScope,
    types::{ContentMatchSummary, SearchPreview},
};

const SEARCH_CANCELLED_MESSAGE: &str = "Search cancelled.";

/// Files larger than this threshold are skipped during content scanning.
/// This prevents very large files (e.g. 150 MB CSV exports) from stalling
/// the search on the main blocking thread. Filename matching still works
/// for oversized files because candidates are collected separately.
const MAX_FILE_SCAN_BYTES: u64 = 10 * 1024 * 1024; // 10 MB

/// Lists all files within the search scope using the `ignore` crate,
/// respecting the current ignore rules while collecting file candidates.
pub fn list_scope_files(
    scope: &SearchScope,
    cancelled: &AtomicBool,
) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    if let Some(slates_root) = &scope.slates_root {
        files.extend(list_directory_files(slates_root, cancelled)?);
    }

    files.extend(scope.local_files.iter().cloned());
    Ok(files)
}

/// Searches file contents for each query term using `grep-searcher` +
/// `grep-regex`.  Returns per-file match summaries keyed by normalised
/// path, with term/document frequency maps needed for BM25 scoring.
///
/// Accepts an already-walked file list to avoid redundant directory
/// traversals — callers should reuse the output of `list_scope_files`.
pub fn collect_content_matches(
    files: &[PathBuf],
    query: &ParsedSearchQuery,
    cancelled: &AtomicBool,
) -> Result<HashMap<String, ContentMatchSummary>, String> {
    let mut by_path: HashMap<String, ContentMatchSummary> = HashMap::new();
    let mut document_frequencies: HashMap<String, usize> = HashMap::new();

    // Build all regex matchers upfront so we can search each file for
    // every term in a single pass over the file list.
    let matchers: Vec<(&str, grep_regex::RegexMatcher)> = query
        .terms
        .iter()
        .map(|term| {
            let escaped = escape_regex_meta(term);
            let matcher = RegexMatcherBuilder::new()
                .case_smart(true)
                .build(&escaped)
                .map_err(|error| format!("Failed to build search matcher: {}", error))?;
            Ok((term.as_str(), matcher))
        })
        .collect::<Result<Vec<_>, String>>()?;

    // Single pass: iterate files once, searching each for all terms.
    // Skip oversized files upfront — one stat check per file, not per term.
    for path in files {
        ensure_not_cancelled(cancelled)?;

        let too_large = std::fs::metadata(path)
            .map(|m| m.len() > MAX_FILE_SCAN_BYTES)
            .unwrap_or(false);
        if too_large {
            continue;
        }

        for (term, matcher) in &matchers {
            if search_file_for_term(path, matcher, term, &mut by_path)? {
                *document_frequencies.entry(term.to_string()).or_insert(0) += 1;
            }
        }
    }

    for summary in by_path.values_mut() {
        summary.document_frequencies = document_frequencies.clone();
    }

    Ok(by_path)
}

// ── Private helpers ──────────────────────────────────────────────────

fn list_directory_files(root: &Path, cancelled: &AtomicBool) -> Result<Vec<PathBuf>, String> {
    ensure_not_cancelled(cancelled)?;

    let mut files = Vec::new();
    for entry in WalkBuilder::new(root).build() {
        if cancelled.load(Ordering::Relaxed) {
            return Err(SEARCH_CANCELLED_MESSAGE.to_string());
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.file_type().map_or(false, |ft| ft.is_file()) {
            files.push(entry.into_path());
        }
    }

    Ok(files)
}

fn search_file_for_term(
    path: &Path,
    matcher: &grep_regex::RegexMatcher,
    term: &str,
    by_path: &mut HashMap<String, ContentMatchSummary>,
) -> Result<bool, String> {
    let path_key = normalize_path_key(path)?;
    let mut collector = MatchCollector {
        term_lower: term.to_lowercase(),
        total_hits: 0,
        preview: None,
    };

    let mut searcher = SearcherBuilder::new().line_number(true).build();

    // search_path gracefully handles binary files (skips them) and
    // unreadable files — both surface as errors we intentionally ignore.
    if searcher.search_path(matcher, path, &mut collector).is_err() {
        return Ok(false);
    }

    if collector.total_hits > 0 {
        let entry = by_path.entry(path_key).or_default();
        entry.total_hits += collector.total_hits;
        *entry.term_frequencies.entry(term.to_string()).or_insert(0) += collector.total_hits;
        if entry.preview.is_none() {
            entry.preview = collector.preview;
        }
        return Ok(true);
    }

    Ok(false)
}

// ── Sink implementation ──────────────────────────────────────────────

struct MatchCollector {
    term_lower: String,
    total_hits: usize,
    preview: Option<SearchPreview>,
}

impl Sink for MatchCollector {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &grep_searcher::Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        let line_text = String::from_utf8_lossy(mat.bytes());

        // Count individual occurrences of the term on this line so
        // term frequency scoring reflects repeated matches.
        let line_lower = line_text.to_lowercase();
        let count = line_lower.matches(&self.term_lower).count().max(1);
        self.total_hits += count;

        if self.preview.is_none() {
            self.preview = Some(SearchPreview {
                line_number: mat.line_number(),
                line_text: line_text.trim_end().to_string(),
            });
        }

        Ok(true)
    }
}

// ── Utilities ────────────────────────────────────────────────────────

/// Escapes regex metacharacters so the term is matched as a literal string.
fn escape_regex_meta(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len() * 2);
    for c in input.chars() {
        if matches!(
            c,
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$'
        ) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

fn ensure_not_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(SEARCH_CANCELLED_MESSAGE.to_string());
    }
    Ok(())
}
