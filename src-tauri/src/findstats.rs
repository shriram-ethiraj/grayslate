use regex::Regex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Maximum number of match positions to cache for exact currentMatch lookup.
/// 1M matches × 16 bytes ≈ 16 MB — acceptable for a desktop app.
const MATCH_CACHE_LIMIT: usize = 1_000_000;

/// Check the cancellation flag every N matches.
const CANCEL_CHECK_INTERVAL: usize = 10_000;

/// Safety valve: stop scanning after this duration and mark results as approximate.
const MAX_SCAN_DURATION_MS: u128 = 5_000;

pub(crate) struct FindStatsOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub use_regex: bool,
}

#[derive(Debug)]
pub(crate) struct FindStatsResult {
    pub match_count: usize,
    pub current_match: usize,
    pub approximate: bool,
}

#[derive(Debug)]
struct MatchRange {
    from: usize,
    to: usize,
}

#[derive(Debug)]
pub(crate) struct ScanCache {
    pub match_count: usize,
    matches: Vec<MatchRange>,
    pub approximate: bool,
}

fn build_regex(search: &str, options: &FindStatsOptions) -> Result<Regex, String> {
    if search.is_empty() {
        return Err("Empty search string".to_string());
    }

    let pattern = if options.use_regex {
        search.to_string()
    } else {
        regex::escape(search)
    };

    let pattern = if options.whole_word {
        format!(r"\b(?:{})\b", pattern)
    } else {
        pattern
    };

    regex::RegexBuilder::new(&pattern)
        .case_insensitive(!options.case_sensitive)
        .build()
        .map_err(|e| format!("Invalid regex: {}", e))
}

/// Scan the full document text for matches, returning stats and a reusable cache.
pub(crate) fn scan(
    text: &str,
    search: &str,
    options: &FindStatsOptions,
    selection_from: usize,
    selection_to: usize,
    cancelled: &AtomicBool,
) -> Result<(FindStatsResult, ScanCache), String> {
    if cancelled.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    let regex = build_regex(search, options)?;

    if cancelled.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    let mut matches = Vec::new();
    let mut match_count = 0usize;
    let mut approximate = false;
    let scan_start = Instant::now();

    for mat in regex.find_iter(text) {
        match_count += 1;

        if matches.len() < MATCH_CACHE_LIMIT {
            matches.push(MatchRange {
                from: mat.start(),
                to: mat.end(),
            });
        }

        if match_count % CANCEL_CHECK_INTERVAL == 0 {
            if cancelled.load(Ordering::Relaxed) {
                return Err("Cancelled".to_string());
            }
            if scan_start.elapsed().as_millis() > MAX_SCAN_DURATION_MS {
                approximate = true;
                break;
            }
        }
    }

    if cancelled.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    let has_all_matches = !approximate && matches.len() == match_count;
    let current_match = if has_all_matches {
        compute_current_match(&matches, selection_from, selection_to)
    } else {
        0
    };

    let cache = ScanCache {
        match_count,
        matches,
        approximate,
    };

    Ok((
        FindStatsResult {
            match_count,
            current_match,
            approximate,
        },
        cache,
    ))
}

/// Binary-search the cached match positions to find the 1-based current match
/// that overlaps or follows the selection.
fn compute_current_match(
    matches: &[MatchRange],
    selection_from: usize,
    selection_to: usize,
) -> usize {
    if matches.is_empty() {
        return 0;
    }

    // Find the first match whose `from` is past the selection end.
    let first_after = matches.partition_point(|m| m.from <= selection_to);

    // Check the match just before first_after for overlap with selection.
    if first_after > 0 {
        let candidate = &matches[first_after - 1];
        if candidate.from <= selection_to && candidate.to > selection_from {
            return first_after; // 1-based
        }
    }

    // No overlap: return the next match after selection.
    if first_after < matches.len() {
        return first_after + 1; // 1-based
    }

    // Wrap around to the first match.
    1
}

/// Recompute currentMatch from cached scan results without rescanning.
pub(crate) fn current_match_from_cache(
    cache: &ScanCache,
    selection_from: usize,
    selection_to: usize,
) -> FindStatsResult {
    let has_all_matches = !cache.approximate && cache.matches.len() == cache.match_count;
    if !has_all_matches {
        return FindStatsResult {
            match_count: cache.match_count,
            current_match: 0,
            approximate: cache.approximate,
        };
    }

    FindStatsResult {
        match_count: cache.match_count,
        current_match: compute_current_match(&cache.matches, selection_from, selection_to),
        approximate: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    fn no_cancel() -> AtomicBool {
        AtomicBool::new(false)
    }

    fn default_options() -> FindStatsOptions {
        FindStatsOptions {
            case_sensitive: false,
            whole_word: false,
            use_regex: false,
        }
    }

    #[test]
    fn literal_case_insensitive() {
        let c = no_cancel();
        let (r, _) = scan("Hello hello HELLO", "hello", &default_options(), 0, 0, &c).unwrap();
        assert_eq!(r.match_count, 3);
        assert!(!r.approximate);
    }

    #[test]
    fn literal_case_sensitive() {
        let c = no_cancel();
        let opts = FindStatsOptions { case_sensitive: true, ..default_options() };
        let (r, _) = scan("Hello hello HELLO", "hello", &opts, 0, 0, &c).unwrap();
        assert_eq!(r.match_count, 1);
    }

    #[test]
    fn whole_word_match() {
        let c = no_cancel();
        let opts = FindStatsOptions { whole_word: true, ..default_options() };
        let (r, _) = scan("hello helloworld hello", "hello", &opts, 0, 0, &c).unwrap();
        assert_eq!(r.match_count, 2);
    }

    #[test]
    fn regex_mode() {
        let c = no_cancel();
        let opts = FindStatsOptions { use_regex: true, ..default_options() };
        let (r, _) = scan("foo123 bar456 baz", r"\d+", &opts, 0, 0, &c).unwrap();
        assert_eq!(r.match_count, 2);
    }

    #[test]
    fn regex_invalid() {
        let c = no_cancel();
        let opts = FindStatsOptions { use_regex: true, ..default_options() };
        let result = scan("text", r"[invalid", &opts, 0, 0, &c);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid regex"));
    }

    #[test]
    fn current_match_at_selection() {
        let c = no_cancel();
        // "aa bb aa bb aa"
        //  0123456789012345
        // "aa" at 0, 6, 12
        let (r, _) = scan("aa bb aa bb aa", "aa", &default_options(), 6, 8, &c).unwrap();
        assert_eq!(r.match_count, 3);
        assert_eq!(r.current_match, 2); // second "aa" at pos 6
    }

    #[test]
    fn current_match_wraps() {
        let c = no_cancel();
        // Selection past last match should wrap to first
        let (r, _) = scan("aa bb aa", "aa", &default_options(), 8, 8, &c).unwrap();
        assert_eq!(r.match_count, 2);
        assert_eq!(r.current_match, 1);
    }

    #[test]
    fn empty_search() {
        let c = no_cancel();
        let result = scan("text", "", &default_options(), 0, 0, &c);
        assert!(result.is_err());
    }

    #[test]
    fn no_matches() {
        let c = no_cancel();
        let (r, _) = scan("hello world", "xyz", &default_options(), 0, 0, &c).unwrap();
        assert_eq!(r.match_count, 0);
        assert_eq!(r.current_match, 0);
    }

    #[test]
    fn selection_only_from_cache() {
        let c = no_cancel();
        let (_, cache) = scan("aa bb aa bb aa", "aa", &default_options(), 0, 0, &c).unwrap();
        let r = current_match_from_cache(&cache, 6, 8);
        assert_eq!(r.current_match, 2);
    }

    #[test]
    fn cancellation() {
        let cancelled = AtomicBool::new(true);
        let result = scan("text", "t", &default_options(), 0, 0, &cancelled);
        assert!(result.is_err());
    }

    #[test]
    fn multiline_literal() {
        let c = no_cancel();
        let (r, _) = scan("line1\nline2\nline1\n", "line1", &default_options(), 0, 0, &c).unwrap();
        assert_eq!(r.match_count, 2);
    }

    #[test]
    fn regex_whole_word() {
        let c = no_cancel();
        let opts = FindStatsOptions { use_regex: true, whole_word: true, ..default_options() };
        let (r, _) = scan("foo foobar foo", r"foo", &opts, 0, 0, &c).unwrap();
        assert_eq!(r.match_count, 2);
    }

    #[test]
    fn literal_with_regex_chars() {
        let c = no_cancel();
        // In literal mode, "a.b" should NOT match "a+b"
        let (r, _) = scan("a.b a+b a.b", "a.b", &default_options(), 0, 0, &c).unwrap();
        assert_eq!(r.match_count, 2);
    }

    #[test]
    fn case_insensitive_whole_word() {
        let c = no_cancel();
        let opts = FindStatsOptions {
            case_sensitive: false,
            whole_word: true,
            use_regex: false,
        };
        let (r, _) = scan("Hello HELLO helloworld", "hello", &opts, 0, 0, &c).unwrap();
        assert_eq!(r.match_count, 2);
    }

    #[test]
    fn current_match_exact_overlap() {
        let c = no_cancel();
        // Selection exactly on a match
        let (r, _) = scan("abc def abc", "abc", &default_options(), 0, 3, &c).unwrap();
        assert_eq!(r.match_count, 2);
        assert_eq!(r.current_match, 1);
    }

    #[test]
    fn current_match_between_matches() {
        let c = no_cancel();
        // Selection between two matches
        let (r, _) = scan("abc def abc", "abc", &default_options(), 4, 7, &c).unwrap();
        assert_eq!(r.match_count, 2);
        assert_eq!(r.current_match, 2); // next match after selection
    }
}
