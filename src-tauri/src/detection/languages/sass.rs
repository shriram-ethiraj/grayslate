use super::{wp, LanguageDefinition};
use regex::Regex;
use std::sync::LazyLock;

fn count_matches(lines: &[&str], re: &Regex) -> usize {
    lines.iter().filter(|l| re.is_match(l)).count()
}

/// Structural detection for Sass (indented syntax, no braces).
pub(crate) fn is_likely_sass(trimmed: &str, _was_sliced: bool) -> bool {
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

    // Sass uses indented syntax (no braces)
    if has_braces {
        return false;
    }

    static INDENT_PROP: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s{2,}[a-z\-]+\s*:\s*[^;{}]+\s*$").unwrap());
    let indent_count = count_matches(&lines, &INDENT_PROP);

    static SASS_VAR_NO_SEMI: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*\$[\w\-]+\s*:\s*[^;{}]+\s*$").unwrap());
    let sass_var_count = count_matches(&lines, &SASS_VAR_NO_SEMI);

    indent_count >= 1 || sass_var_count >= 2 || at_count >= 1
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "sass",
        extensions: &[".sass"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(91),
        structural_detect: Some(is_likely_sass),
        patterns: &[
            wp!(r"(?m)^\$[\w-]+:", 4),
            wp!(r"(?m)^=[\w-]+", 4),
            wp!(r"(?m)^\+[\w-]+", 4),
            wp!(r"@mixin\s+[\w-]+", 3),
            wp!(r"@include\s+[\w-]+", 3),
            wp!(r"@extend\s+[.%]", 3),
            wp!(r"@import\s+", 2),
            wp!(r"@use\s+", 3),
            wp!(r"@forward\s+", 3),
            wp!(r"@function\s+[\w-]+", 3),
            wp!(r"@return\s+", 2),
            wp!(r"@if\s+", 2),
            wp!(r"@each\s+", 2),
            wp!(r"@for\s+", 2),
        ],
        anti_patterns: &[
            wp!(r"\{", -3),
            wp!(r";$", -2),
        ],
        uses_hash_comments: false,
        keywords: &[
            "@mixin", "@include", "@extend", "@function", "@return", "@if", "@else",
            "@each", "@for", "@while", "@use", "@forward", "@import", "@at-root",
            "@debug", "@warn", "@error",
        ],
        builtins: &[
            "lighten", "darken", "saturate", "desaturate", "mix", "rgba", "hsla",
            "nth", "map-get", "percentage", "round", "ceil", "floor", "abs", "min",
            "max",
        ],
        family: None,
        exclusive_patterns: &[],
    }
}
