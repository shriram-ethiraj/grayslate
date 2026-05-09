use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::query::SearchOptions;

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
    pub size_bytes: Option<u64>,
    pub file_modified_app_at: Option<i64>,
    pub file_modified_disk_at: Option<i64>,
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
    pub size_bytes: Option<u64>,
    pub file_modified_app_at: Option<i64>,
    pub file_modified_disk_at: Option<i64>,
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
pub fn get_line_excerpt(line_text: &str, terms: &[String], options: &SearchOptions, is_glob: bool) -> String {
    let trimmed = line_text.trim();
    if trimmed.len() <= LINE_EXCERPT_MAX_LENGTH || terms.is_empty() {
        if trimmed.len() > LINE_EXCERPT_MAX_LENGTH {
            let end = floor_char_boundary(trimmed, LINE_EXCERPT_MAX_LENGTH);
            return format!("{}…", &trimmed[..end]);
        }
        return trimmed.to_string();
    }

    let mut match_start: Option<usize> = None;

    if is_glob {
        // Glob mode never produces content matches — safety fallback.
        let end = floor_char_boundary(trimmed, LINE_EXCERPT_MAX_LENGTH.min(trimmed.len()));
        return if trimmed.len() > LINE_EXCERPT_MAX_LENGTH {
            format!("{}…", &trimmed[..end])
        } else {
            trimmed.to_string()
        };
    }

    if options.use_regex {
        // In regex mode, use a compiled regex to locate the first match.
        if let Some(pattern) = terms.first() {
            if let Ok(re) = regex::RegexBuilder::new(pattern)
                .case_insensitive(!options.case_sensitive)
                .build()
            {
                if let Some(m) = re.find(trimmed) {
                    match_start = Some(m.start());
                }
            }
        }
    } else {
        // Literal mode: find earliest term occurrence.
        let haystack = if options.case_sensitive {
            trimmed.to_string()
        } else {
            trimmed.to_lowercase()
        };
        for term in terms {
            if term.is_empty() {
                continue;
            }
            let found = if options.whole_word {
                find_whole_word(&haystack, term.as_str())
            } else {
                haystack.find(term.as_str())
            };
            if let Some(idx) = found {
                match_start = Some(match match_start {
                    Some(prev) if prev <= idx => prev,
                    _ => idx,
                });
            }
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
/// given `terms`.  Terms are tried longest-first so that a longer term is
/// preferred over a shorter prefix.
///
/// In **glob mode** (`is_glob = true`) the query is a path filter — inline
/// highlighting doesn't apply so all text is returned plain.
///
/// In **regex mode** the first term is compiled as a regex and match spans
/// are used directly.
///
/// In **literal mode** terms are matched as substrings with case and
/// whole-word awareness.
pub fn split_text_by_terms(
    text: &str,
    terms: &[String],
    options: &SearchOptions,
    is_glob: bool,
) -> Vec<HighlightFragment> {
    if text.is_empty() || terms.is_empty() {
        return vec![HighlightFragment {
            text: text.to_string(),
            is_match: false,
        }];
    }

    if is_glob {
        let glob_literals = extract_glob_highlight_literals(
            terms.first().map(|s| s.as_str()).unwrap_or(""),
            options.case_sensitive,
        );
        if glob_literals.is_empty() {
            return vec![HighlightFragment {
                text: text.to_string(),
                is_match: false,
            }];
        }
        return split_text_by_literals(text, &glob_literals, options.case_sensitive, false);
    }

    if options.use_regex {
        split_text_by_regex(text, terms, options.case_sensitive)
    } else {
        split_text_by_literals(text, terms, options.case_sensitive, options.whole_word)
    }
}

/// Regex-mode highlight: compile the first term as a regex and split text
/// around every non-overlapping match.
fn split_text_by_regex(
    text: &str,
    terms: &[String],
    case_sensitive: bool,
) -> Vec<HighlightFragment> {
    let pattern = match terms.first() {
        Some(p) if !p.is_empty() => p.as_str(),
        _ => {
            return vec![HighlightFragment {
                text: text.to_string(),
                is_match: false,
            }]
        }
    };

    let re = match regex::RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .build()
    {
        Ok(r) => r,
        Err(_) => {
            return vec![HighlightFragment {
                text: text.to_string(),
                is_match: false,
            }]
        }
    };

    let mut fragments: Vec<HighlightFragment> = Vec::new();
    let mut cursor = 0;

    for mat in re.find_iter(text) {
        if mat.start() > cursor {
            fragments.push(HighlightFragment {
                text: text[cursor..mat.start()].to_string(),
                is_match: false,
            });
        }
        if mat.start() < mat.end() {
            fragments.push(HighlightFragment {
                text: text[mat.start()..mat.end()].to_string(),
                is_match: true,
            });
        }
        cursor = mat.end();
    }

    if cursor < text.len() {
        fragments.push(HighlightFragment {
            text: text[cursor..].to_string(),
            is_match: false,
        });
    }

    if fragments.is_empty() {
        return vec![HighlightFragment {
            text: text.to_string(),
            is_match: false,
        }];
    }

    fragments
}

/// Literal-mode highlight: case-aware substring matching with longest-term
/// priority (original algorithm, extended for case sensitivity and whole-word).
fn split_text_by_literals(
    text: &str,
    terms: &[String],
    case_sensitive: bool,
    whole_word: bool,
) -> Vec<HighlightFragment> {
    // Sort terms longest-first so longer matches take priority.
    let mut sorted_terms: Vec<&str> = terms.iter().map(|t| t.as_str()).collect();
    sorted_terms.sort_by(|a, b| b.len().cmp(&a.len()));

    let haystack = if case_sensitive {
        text.to_string()
    } else {
        text.to_lowercase()
    };
    let mut fragments: Vec<HighlightFragment> = Vec::new();
    let mut cursor = 0;

    while cursor < text.len() {
        let remaining = &haystack[cursor..];
        let mut best_match: Option<(usize, usize)> = None; // (offset, length)

        for term in &sorted_terms {
            if term.is_empty() {
                continue;
            }
            if let Some(offset) = if whole_word {
                find_whole_word(remaining, term.as_ref() as &str)
            } else {
                remaining.find(term.as_ref() as &str)
            } {
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
                    // Use original text casing for the displayed fragment.
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

/// Extracts literal segments from a glob pattern for highlighting.
/// Expands `{a,b}` braces first so `*.{csv,txt}` yields `[".csv", ".txt"]`
/// instead of `["."]`, `["csv"]`, `["txt"]`.  Then splits on wildcards
/// (`*`/`?`) and strips `[...]` character classes.
fn extract_glob_highlight_literals(pattern: &str, case_sensitive: bool) -> Vec<String> {
    let expanded = expand_braces(pattern);
    let mut literals: Vec<String> = Vec::new();
    for pat in &expanded {
        for segment in pat.split(|c: char| c == '*' || c == '?') {
            let cleaned = strip_bracket_classes(segment);
            if cleaned.is_empty() {
                continue;
            }
            let lit = if case_sensitive {
                cleaned
            } else {
                cleaned.to_lowercase()
            };
            if !literals.contains(&lit) {
                literals.push(lit);
            }
        }
    }
    // Longest-first so longer matches take priority in highlighting.
    literals.sort_by(|a, b| b.len().cmp(&a.len()));
    literals
}

/// Recursively expands `{a,b,c}` brace expressions.
/// `*.{csv,txt}` → `["*.csv", "*.txt"]`.
fn expand_braces(pattern: &str) -> Vec<String> {
    if let Some(open) = pattern.find('{') {
        if let Some(close) = pattern[open..].find('}').map(|i| open + i) {
            let prefix = &pattern[..open];
            let suffix = &pattern[close + 1..];
            return pattern[open + 1..close]
                .split(',')
                .flat_map(|alt| expand_braces(&format!("{}{}{}", prefix, alt.trim(), suffix)))
                .collect();
        }
    }
    vec![pattern.to_string()]
}

/// Removes `[...]` character class sequences, keeping only plain text.
fn strip_bracket_classes(s: &str) -> String {
    let mut result = String::new();
    let mut in_bracket = false;
    for c in s.chars() {
        if c == '[' {
            in_bracket = true;
            continue;
        }
        if c == ']' {
            in_bracket = false;
            continue;
        }
        if !in_bracket {
            result.push(c);
        }
    }
    result
}

/// Finds the first occurrence of `needle` in `haystack` that sits at a word
/// boundary on both sides (alphanumeric/underscore = word char).  Returns the
/// byte offset into `haystack`, or `None` if no whole-word match exists.
///
/// Safe for Unicode strings: the char before/after the match is checked via
/// `chars()` so we never slice at a non-boundary.
pub fn find_whole_word(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    let mut start = 0;
    while start < haystack.len() {
        if !haystack.is_char_boundary(start) {
            start += 1;
            continue;
        }
        match haystack[start..].find(needle) {
            None => break,
            Some(offset) => {
                let pos = start + offset;
                let end = pos + needle.len();
                let before_ok = pos == 0
                    || haystack[..pos]
                        .chars()
                        .next_back()
                        .map_or(true, |c| !is_word_char(c));
                let after_ok = end >= haystack.len()
                    || haystack[end..]
                        .chars()
                        .next()
                        .map_or(true, |c| !is_word_char(c));
                if before_ok && after_ok {
                    return Some(pos);
                }
                // Not a word boundary here; advance past this occurrence.
                start = pos + 1;
            }
        }
    }
    None
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_braces_single() {
        assert_eq!(expand_braces("*.csv"), vec!["*.csv"]);
    }

    #[test]
    fn expand_braces_pair() {
        let mut result = expand_braces("*.{csv,txt}");
        result.sort();
        assert_eq!(result, vec!["*.csv", "*.txt"]);
    }

    #[test]
    fn expand_braces_triple() {
        let mut result = expand_braces("*.{csv,txt,json}");
        result.sort();
        assert_eq!(result, vec!["*.csv", "*.json", "*.txt"]);
    }

    #[test]
    fn strip_bracket_classes_basic() {
        assert_eq!(strip_bracket_classes("test[0-9].py"), "test.py");
        assert_eq!(strip_bracket_classes("no_brackets"), "no_brackets");
        assert_eq!(strip_bracket_classes("[A-Z]file"), "file");
    }

    #[test]
    fn glob_literals_brace_expansion() {
        let lits = extract_glob_highlight_literals("*.{csv,txt}", false);
        assert_eq!(lits, vec![".csv", ".txt"]);
    }

    #[test]
    fn glob_literals_simple() {
        let lits = extract_glob_highlight_literals("*.csv", false);
        assert_eq!(lits, vec![".csv"]);
    }

    #[test]
    fn glob_literals_with_char_class() {
        let lits = extract_glob_highlight_literals("[A-Z]*.py", false);
        assert_eq!(lits, vec![".py"]);
    }

    #[test]
    fn glob_literals_case_sensitive() {
        let lits = extract_glob_highlight_literals("*.CSV", true);
        assert_eq!(lits, vec![".CSV"]);
    }

    #[test]
    fn glob_highlight_csv_txt_whole_match() {
        let options = SearchOptions {
            case_sensitive: false,
            whole_word: false,
            use_regex: false,
        };
        let frags = split_text_by_terms("report.csv", &["*.{csv,txt}".into()], &options, true);
        let matched: Vec<_> = frags.iter().filter(|f| f.is_match).map(|f| f.text.as_str()).collect();
        assert_eq!(matched, vec![".csv"]);
    }

    #[test]
    fn glob_highlight_tsv_brace() {
        let options = SearchOptions {
            case_sensitive: false,
            whole_word: false,
            use_regex: false,
        };
        let frags = split_text_by_terms("data.tsv", &["*.{csv,tsv}".into()], &options, true);
        let matched: Vec<_> = frags.iter().filter(|f| f.is_match).map(|f| f.text.as_str()).collect();
        assert_eq!(matched, vec![".tsv"]);
    }
}
