use super::LanguageDefinition;
use regex::Regex;
use std::sync::LazyLock;

fn count_matches(lines: &[&str], re: &Regex) -> usize {
    lines.iter().filter(|l| re.is_match(l)).count()
}

/// Strip content inside double quotes to avoid counting grammatical delimiters.
fn strip_quoted(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_quotes = false;
    for ch in s.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if !in_quotes {
            result.push(ch);
        }
    }
    result
}

/// Returns true when >= 80% of sampled lines share the same delimiter count
/// as the header row, with at least 1 delimiter per line.
fn has_consistent_delimiter(lines: &[&str], delimiter: &str) -> bool {
    let clean_header = strip_quoted(lines[0]);
    let header_count = clean_header.matches(delimiter).count();

    if header_count < 1 {
        return false;
    }

    // Pipe delimiter: exclude markdown tables (every line starts & ends with |)
    if delimiter == "|" {
        let all_table = lines
            .iter()
            .all(|l| l.starts_with('|') && l.ends_with('|'));
        if all_table {
            return false;
        }
    }

    let sample = &lines[..lines.len().min(20)];
    let matching = sample
        .iter()
        .filter(|l| {
            let clean = strip_quoted(l);
            clean.matches(delimiter).count() == header_count
        })
        .count();

    matching as f64 / sample.len() as f64 >= 0.8
}

pub(crate) fn is_likely_csv(trimmed: &str, _was_sliced: bool) -> bool {
    let first = trimmed.as_bytes().first().copied().unwrap_or(0);
    if first == b'{' || first == b'[' || first == b'<' {
        return false;
    }

    let lines: Vec<&str> = trimmed
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    // Require header + at least 3 data rows
    if lines.len() < 4 {
        return false;
    }

    // If most lines look like YAML key: value, skip CSV
    static YAML_KV: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[a-zA-Z_][\w.\-]*\s*:\s").unwrap());
    let yaml_count = count_matches(&lines, &YAML_KV);
    if yaml_count as f64 / lines.len() as f64 > 0.5 {
        return false;
    }

    // If most lines look like script/source code, reject CSV
    static SCRIPT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^\s*(#|//|echo|import|from|const|let|var|def|class|function|export|@import|@forward|@use|@mixin|@include|use\s+\w+::)\b",
        )
        .unwrap()
    });
    let script_count = count_matches(&lines, &SCRIPT);
    if script_count as f64 / lines.len() as f64 > 0.3 {
        return false;
    }

    // Curly braces — likely CSS, JS, Rust, etc.
    let brace_count = lines
        .iter()
        .filter(|l| l.contains('{') || l.contains('}'))
        .count();
    if brace_count as f64 / lines.len() as f64 > 0.2 {
        return false;
    }

    // CSS/SCSS selectors have commas between selectors — reject if @-rules present
    static CSS_AT_RULES: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^\s*@(import|forward|use|media|mixin|include|keyframes|charset)\b").unwrap()
    });
    if lines.iter().any(|l| CSS_AT_RULES.is_match(l)) {
        return false;
    }

    for delim in &[",", "\t", ";", "|"] {
        if has_consistent_delimiter(&lines, delim) {
            return true;
        }
    }
    false
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "csv",
        extensions: &[".csv", ".tsv"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(70),
        structural_detect: Some(is_likely_csv),
        patterns: &[],
        anti_patterns: &[],
        uses_hash_comments: false,
        keywords: &[],
        builtins: &[],
        illegal: None,
        extends: None,
    }
}
