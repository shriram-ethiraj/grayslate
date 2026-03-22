use super::LanguageDefinition;
use regex::Regex;
use std::sync::LazyLock;

/// Structural detection for JSON, JSONL, and JSONC.
pub(crate) fn is_likely_json(trimmed: &str, was_sliced: bool) -> bool {
    let first = match trimmed.as_bytes().first() {
        Some(b) => *b,
        None => return false,
    };
    if first != b'{' && first != b'[' {
        return false;
    }

    // Authoritative parse (only when we have the complete content)
    if !was_sliced {
        if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
            return true;
        }
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
            serde_json::from_str::<serde_json::Value>(line).is_ok()
        });
        if all_json {
            return true;
        }
    }

    // Structural heuristic for sliced / JSONC content
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
        patterns: &[],
        anti_patterns: &[],
        uses_hash_comments: false,
        keywords: &[],
        builtins: &[],
        family: None,
        exclusive_patterns: &[],
    }
}
