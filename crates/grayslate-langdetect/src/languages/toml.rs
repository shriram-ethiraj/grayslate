use super::{wp, LanguageDefinition};
use super::ContentFamily;
use regex::Regex;
use std::sync::LazyLock;

fn count_matches(lines: &[&str], re: &Regex) -> usize {
    lines.iter().filter(|l| re.is_match(l)).count()
}

pub(crate) fn is_likely_toml(trimmed: &str, _was_sliced: bool) -> bool {
    let first = trimmed.as_bytes().first().copied().unwrap_or(0);
    if first == b'<' {
        return false;
    }
    if first == b'{' {
        return false;
    }
    if first == b'[' {
        let second = trimmed.as_bytes().get(1).copied().unwrap_or(0);
        if second == b'"' {
            return false;
        }
    }

    let lines: Vec<&str> = trimmed.lines().collect();
    let non_empty: Vec<&str> = lines
        .iter()
        .copied()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if non_empty.len() < 2 {
        return false;
    }

    static SASS_VAR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*\$[\w\-]+\s*:").unwrap());
    if non_empty.iter().any(|l| SASS_VAR.is_match(l)) {
        return false;
    }

    static CODE_SIGNALS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?x)
            ^\s*(
                import\s | from\s+[\w.]+\s+import | export\s |
                const\s | let\s | var\s |
                func\s | fn\s | def\s | class\s |
                package\s | pub\s+(fn|struct|enum|mod|trait|impl|use)\s |
                use\s+\w+(::\w+)+ |
                return\s | if\s+\w | for\s | while\s |
                async\s | await\s
            )
        ").unwrap()
    });
    let code_count = non_empty.iter().filter(|l| CODE_SIGNALS.is_match(l)).count();
    if code_count as f64 / non_empty.len() as f64 > 0.10 {
        return false;
    }

    static JSX_SIGNALS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?x)
            ^\s*"use\s+(client|server)" |
            <[A-Z]\w+ |
            />\s*$ |
            </[A-Z]\w+ |
            className= |
            \w+=\{
        "#).unwrap()
    });
    let jsx_count = non_empty.iter().filter(|l| JSX_SIGNALS.is_match(l)).count();
    if jsx_count >= 1 {
        return false;
    }

    static ARROW_FN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"=>\s*[\{(\n]|=>\s*$").unwrap());
    if non_empty.iter().any(|l| ARROW_FN.is_match(l)) {
        return false;
    }

    let semi_count = non_empty.iter().filter(|l| l.ends_with(';')).count();
    if semi_count as f64 / non_empty.len() as f64 > 0.2 {
        return false;
    }

    let cpp_comment_count = non_empty
        .iter()
        .filter(|l| l.starts_with("//") || l.contains(" //"))
        .count();
    if cpp_comment_count as f64 / non_empty.len() as f64 > 0.15 {
        return false;
    }

    static RUBY_GUARD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?x)
            ^\s*end\s*$ |
            \bdo\s*\| |
            \#\{[^}]+\}
        ").unwrap()
    });
    let ruby_count = non_empty.iter().filter(|l| RUBY_GUARD.is_match(l)).count();
    if ruby_count >= 2 {
        return false;
    }

    static PY_SIGNALS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?x)
            ^\s*(
                assert\s | @\w+[\.(] | raise\s | except\s |
                if\s+__name__\s*== | print\s*\( | pytest\. |
                def\s+\w+ | class\s+\w+ | return\s
            )
        ").unwrap()
    });
    let py_count = non_empty.iter().filter(|l| PY_SIGNALS.is_match(l)).count();
    if py_count >= 2 {
        return false;
    }

    let paren_end_count = non_empty
        .iter()
        .filter(|l| l.ends_with(')') || l.ends_with("})") || l.ends_with("},"))
        .count();
    if paren_end_count as f64 / non_empty.len() as f64 > 0.15 {
        return false;
    }

    let mut score = 0i32;

    static SECTION: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*\[\[?[\w.\-]+\]\]?\s*$").unwrap());
    let section_count = count_matches(&non_empty, &SECTION);
    if section_count >= 1 {
        score += 2;
    }

    static KV: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*[\w.\-]+\s*=\s*(.+)$").unwrap());
    let mut kv_count = 0;
    for line in &non_empty {
        if let Some(caps) = KV.captures(line) {
            kv_count += 1;
            let val = caps.get(1).unwrap().as_str().trim();
            if val.starts_with("\"\"\"") || val.starts_with("'''") {
                score += 1;
            }
            if val.starts_with('[') || val.starts_with('{') {
                score += 1;
            }
            if val == "true" || val == "false" {
                score += 1;
            }
            static DATETIME: LazyLock<Regex> =
                LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}").unwrap());
            if DATETIME.is_match(val) {
                score += 2;
            }
        }
    }

    if kv_count >= 2 {
        score += 1;
    }

    static COLON_KV: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*[\w.\-]+\s*:\s").unwrap());
    let colon_count = count_matches(&non_empty, &COLON_KV);
    if colon_count > kv_count {
        return false;
    }

    if non_empty.len() >= 5 && (kv_count as f64 / non_empty.len() as f64) < 0.25 {
        return false;
    }

    let threshold = if section_count >= 1 { 3 } else { 4 };
    score >= threshold
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "toml",
        extensions: &[".toml"],
        filenames: &["cargo.toml", "cargo.lock"],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(100),
        structural_detect: Some(is_likely_toml),
        keywords: &[],
        builtins: &[],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::StructuredData, ContentFamily::Config],
        anchors: &[
            wp!(r"(?m)^\s*\[[\w.\-]+\]\s*$", 5),
            wp!(r"(?m)^\s*\[\[[\w.\-]+\]\]\s*$", 5),
            wp!(r#"(?m)^\s*[\w.\-]+\s*=\s*""#, 4),
        ],
        hints: &[
            wp!(r"(?m)^\s*[\w.\-]+\s*=\s*(true|false)\b", 3),
            wp!(r"(?m)^\s*[\w.\-]+\s*=\s*\d{4}-\d{2}-\d{2}", 3),
        ],
        disqualifiers: &[
            wp!(r"(?m)^\s*[a-zA-Z_]\w*\s*:\s+\S", -4),
        ],
    }
}
