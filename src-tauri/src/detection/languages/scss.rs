use super::{wp, LanguageDefinition};
use regex::Regex;
use std::sync::LazyLock;

fn count_matches(lines: &[&str], re: &Regex) -> usize {
    lines.iter().filter(|l| re.is_match(l)).count()
}

/// Structural detection for SCSS (braces syntax).
pub(crate) fn is_likely_scss(trimmed: &str, _was_sliced: bool) -> bool {
    let first = trimmed.as_bytes().first().copied().unwrap_or(0);
    if first == b'<' || first == b'{' || first == b'[' {
        return false;
    }

    let lines: Vec<&str> = trimmed
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//") && !t.starts_with('#')
        })
        .collect();

    if lines.len() < 2 {
        return false;
    }

    static SASS_VAR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*\$[\w\-]+\s*:\s*.+;?\s*$").unwrap());
    let var_count = count_matches(&lines, &SASS_VAR);

    static SASS_AT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*@(mixin|include|extend|use|forward)\b").unwrap());
    let at_count = count_matches(&lines, &SASS_AT);

    if var_count < 1 && at_count < 1 {
        return false;
    }

    let has_braces = lines.iter().any(|l| l.contains('{') || l.contains('}'));
    let semi_count = lines.iter().filter(|l| l.trim().ends_with(';')).count();

    static CSS_SELECTOR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"([.#][\w\-]+|[a-z][\w\-]*)\s*\{\s*$").unwrap());
    let selector_count = lines
        .iter()
        .filter(|l| CSS_SELECTOR.is_match(l.trim()))
        .count();

    let scss_score = (if has_braces { 2 } else { 0 })
        + (if semi_count >= 2 {
            2
        } else if semi_count >= 1 {
            1
        } else {
            0
        })
        + (if selector_count >= 1 { 1 } else { 0 })
        + (if at_count >= 1 && has_braces { 2 } else { 0 });

    scss_score >= 2
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "scss",
        extensions: &[".scss"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(90),
        structural_detect: Some(is_likely_scss),
        patterns: &[
            wp!(r"\$[\w-]+\s*:", 4),
            wp!(r"@mixin\s+[\w-]+", 4),
            wp!(r"@include\s+[\w-]+", 4),
            wp!(r"@extend\s+[.%]", 3),
            wp!(r#"@import\s+['"]"#, 2),
            wp!(r#"@use\s+['"]"#, 3),
            wp!(r#"@forward\s+['"]"#, 3),
            wp!(r"@function\s+[\w-]+", 3),
            wp!(r"@return\s+", 2),
            wp!(r"@if\s+", 2),
            wp!(r"@each\s+", 2),
            wp!(r"@for\s+", 2),
            wp!(r"&[.:\[>~+]", 2),
            wp!(r"#\{.*?\}", 2),
        ],
        anti_patterns: &[
            wp!(r"^\s*\{", -3),
            wp!(r"(?m)^\s*def\s+", -3),
        ],
        uses_hash_comments: false,
        keywords: &[
            "@mixin", "@include", "@extend", "@function", "@return", "@if", "@else",
            "@each", "@for", "@while", "@use", "@forward", "@import", "@at-root",
            "@debug", "@warn", "@error",
        ],
        builtins: &[
            "lighten", "darken", "saturate", "desaturate", "mix", "rgba", "hsla",
            "nth", "map-get", "map-merge", "percentage", "round", "ceil", "floor",
            "abs", "min", "max", "length", "append", "join", "unquote", "quote",
        ],
        family: None,
        exclusive_patterns: &[],
    }
}
