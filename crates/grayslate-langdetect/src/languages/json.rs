use super::{wp, LanguageDefinition};
use super::ContentFamily;
use regex::Regex;
use std::sync::LazyLock;

/// Lightweight JSON structure validator — checks balanced braces/brackets
/// with proper string/escape handling. Sufficient for detection purposes
/// (distinguishing JSON from code) without needing a full parser.
fn is_balanced_json(content: &str) -> bool {
    let bytes = content.trim().as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let first = bytes[0];
    if first != b'{' && first != b'[' {
        return false;
    }
    let expected_close = if first == b'{' { b'}' } else { b']' };

    // Quick structural sanity: after `{`, next non-ws must be `"` or `}` (valid JSON).
    // This rejects Jinja `{% ... %}`, template `{{ var }}`, etc.
    if first == b'{' {
        if let Some(&next) = bytes[1..].iter().find(|b| !b.is_ascii_whitespace()) {
            if next != b'"' && next != b'}' {
                return false;
            }
        }
    }

    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut i = 0;
    let len = bytes.len();

    while i < len {
        if in_string {
            if bytes[i] == b'\\' {
                i += 2; // skip escape sequence
                continue;
            }
            if bytes[i] == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match bytes[i] {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
        i += 1;
    }

    // Must end balanced, not inside a string, and last non-whitespace
    // must be the matching close bracket/brace.
    if depth != 0 || in_string {
        return false;
    }
    // Verify the last significant byte is the expected closing delimiter.
    bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map_or(false, |pos| bytes[pos] == expected_close)
}

/// Structural detection for JSON, JSONL, and JSONC.
pub(crate) fn is_likely_json(trimmed: &str, was_sliced: bool) -> bool {
    let first = match trimmed.as_bytes().first() {
        Some(b) => *b,
        None => return false,
    };
    if first != b'{' && first != b'[' {
        return false;
    }

    // Authoritative structure check (only when we have the complete content)
    if !was_sliced && is_balanced_json(trimmed) {
        return true;
    }

    // JSONL — each non-empty line is its own JSON value
    let lines: Vec<&str> = trimmed
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.len() >= 2 {
        let sample = &lines[..lines.len().min(5)];
        let all_json = sample.iter().all(|line| {
            let first_byte = line.as_bytes().first().copied().unwrap_or(0);
            if first_byte != b'{' && first_byte != b'[' {
                return false;
            }
            is_balanced_json(line)
        });
        if all_json {
            return true;
        }
    }

    // Structural pattern for sliced / JSONC content
    static JSON_PAIRS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#""[\w$][\w\s$.\-]*"\s*:\s*["\{\[\dtfn\-]"#).unwrap());
    if !JSON_PAIRS.is_match(trimmed) {
        return false;
    }

    static CODE_SIGNAL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(const|let|var|function|class|import|export|module|return)\b").unwrap()
    });

    let first_lines: Vec<&str> = trimmed.lines().take(10).collect();
    let code_count = first_lines.iter().filter(|l| CODE_SIGNAL.is_match(l)).count();
    code_count == 0
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "json",
        extensions: &[".json", ".jsonc", ".json5", ".geojson", ".webmanifest", ".har", ".arb"],
        filenames: &[".prettierrc"],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(5),
        structural_detect: Some(is_likely_json),
        keywords: &[],
        builtins: &[],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::StructuredData],
        anchors: &[
            wp!(r#"(?m)^\s*"[\w$][\w$.\-]*"\s*:\s*["{\[\dtfn]"#, 4),
        ],
        hints: &[
            wp!(r#"(?m)^\s*\},"#, 2),
        ],
        disqualifiers: &[
            wp!(r"(?m)^\s*(import|export|const|let|var|function|class)\s", -5),
        ],
    }
}
