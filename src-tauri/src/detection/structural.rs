/// Phase 3 — Structural signal detection.
///
/// Deterministic checks for formats identifiable by their syntax skeleton.
/// ORDER MATTERS — most-unambiguous formats first to prevent false positives.
///
/// Current order:
///   JSON → PHP → Svelte → Vue → HTML → XML → Dockerfile → CSV
///   → Markdown → Sass/SCSS → TOML → YAML
use regex::Regex;
use std::sync::LazyLock;

/// Try to detect a data-format or markup language from structural signals.
pub fn detect_structural(trimmed: &str, was_sliced: bool) -> Option<&'static str> {
    if is_likely_json(trimmed, was_sliced) {
        return Some("json");
    }
    if is_likely_php(trimmed) {
        return Some("php");
    }
    if is_likely_svelte(trimmed) {
        return Some("svelte");
    }
    if is_likely_vue(trimmed) {
        return Some("vue");
    }
    if is_likely_html(trimmed) {
        return Some("html");
    }
    if is_likely_xml(trimmed) {
        return Some("xml");
    }
    if is_likely_dockerfile(trimmed) {
        return Some("dockerfile");
    }
    if is_likely_csv(trimmed) {
        return Some("csv");
    }
    if is_likely_markdown(trimmed) {
        return Some("markdown");
    }
    if let Some(sass_like) = detect_sass_scss(trimmed) {
        return Some(sass_like);
    }
    if is_likely_toml(trimmed) {
        return Some("toml");
    }
    // YAML after markdown to avoid eating frontmatter
    if is_likely_yaml(trimmed) {
        return Some("yaml");
    }
    None
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn count_matches(lines: &[&str], re: &Regex) -> usize {
    lines.iter().filter(|l| re.is_match(l)).count()
}

// ── 3a. JSON ─────────────────────────────────────────────────────────────

/// Detects JSON, JSONL, and JSONC.
///
/// Strategy:
///   1. Content must start with `{` or `[`.
///   2. If full content available: authoritative serde_json parse.
///   3. JSONL: each non-empty line is independent JSON.
///   4. Structural heuristic: "key": value patterns without code signals.
fn is_likely_json(trimmed: &str, was_sliced: bool) -> bool {
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

// ── 3a.1 PHP ─────────────────────────────────────────────────────────────

fn is_likely_php(trimmed: &str) -> bool {
    static PHP_OPEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^<\?php\b").unwrap());
    if PHP_OPEN.is_match(trimmed) {
        return true;
    }
    // Short open tag with PHP content
    if trimmed.starts_with("<?") && !trimmed.starts_with("<?xml") {
        static PHP_VAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$\w+\s*=").unwrap());
        static PHP_ECHO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\becho\s").unwrap());
        if PHP_VAR.is_match(trimmed) || PHP_ECHO.is_match(trimmed) {
            return true;
        }
    }
    false
}

// ── 3a.2 Svelte ──────────────────────────────────────────────────────────

fn is_likely_svelte(trimmed: &str) -> bool {
    let starts_with_tag = trimmed.starts_with('<');
    let has_block_tag = trimmed.contains("{#");

    if !starts_with_tag && !has_block_tag {
        return false;
    }

    // Anti-signal: JS/TS declarations in first meaningful lines
    if !starts_with_tag {
        static JSTS_CODE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(
                r"^(const|let|var|type|interface|function|class|export|import|async\s+function)\b",
            )
            .unwrap()
        });
        let first_lines: Vec<&str> = trimmed
            .lines()
            .map(|l| l.trim())
            .filter(|l| {
                !l.is_empty()
                    && !l.starts_with('*')
                    && !l.starts_with("//")
                    && !l.starts_with("/*")
            })
            .take(5)
            .collect();
        let code_count = first_lines.iter().filter(|l| JSTS_CODE.is_match(l)).count();
        if code_count >= 2 {
            return false;
        }
    }

    static SVELTE_SIGNALS: LazyLock<Vec<(Regex, i32)>> = LazyLock::new(|| vec![
        (Regex::new(r"\{#(if|each|await|snippet|key)[}\s]").unwrap(), 3),
        (Regex::new(r"\{:(else|then|catch)[}\s]").unwrap(), 3),
        (Regex::new(r"\{/(if|each|await|snippet|key)\}").unwrap(), 3),
        (Regex::new(r#"<script\s+(context="module"|lang="ts")[^>]*>"#).unwrap(), 3),
        (Regex::new(r"\b(bind:|on:|use:|transition:|animate:|let:|class:)[a-zA-Z\-]+=").unwrap(), 2),
        (Regex::new(r"\$(state|derived|effect|props)\(").unwrap(), 4),
        (Regex::new(r"(?m)^\s*\$:\s+").unwrap(), 4),
        (Regex::new(r"<slot[\s>]").unwrap(), 2),
        (Regex::new(r"\{@(html|render|debug|const)\s+").unwrap(), 2),
    ]);

    let mut score = 0i32;
    for (re, weight) in SVELTE_SIGNALS.iter() {
        if re.is_match(trimmed) {
            score += weight;
        }
    }
    score >= 2
}

// ── 3a.3 Vue ─────────────────────────────────────────────────────────────

fn is_likely_vue(trimmed: &str) -> bool {
    if !trimmed.starts_with('<') {
        return false;
    }

    static VUE_SIGNALS: LazyLock<Vec<(Regex, i32)>> = LazyLock::new(|| vec![
        (Regex::new(r"<template[\s>]").unwrap(), 4),
        (Regex::new(r"\b(v-if|v-else-if|v-else|v-show|v-for|v-on:|v-bind:|v-model|v-slot)[=>\s]").unwrap(), 2),
        (Regex::new(r"@(click|submit|input|change|keyup|keydown)=").unwrap(), 2),
        (Regex::new(r":(class|style|value|disabled|key)=").unwrap(), 2),
        (Regex::new(r"<script\s+setup[^>]*>").unwrap(), 3),
        (Regex::new(r"\b(defineProps|defineEmits|defineExpose)\s*\(").unwrap(), 2),
        (Regex::new(r"\b(ref|reactive|computed|watch|onMounted)\s*\(").unwrap(), 2),
    ]);

    let mut score = 0i32;
    for (re, weight) in VUE_SIGNALS.iter() {
        if re.is_match(trimmed) {
            score += weight;
        }
    }
    score >= 4
}

// ── 3b. HTML ─────────────────────────────────────────────────────────────

fn is_likely_html(trimmed: &str) -> bool {
    if trimmed.starts_with("<?xml") {
        return false;
    }

    // Quick Svelte/Vue bail-outs
    static SVELTE_RUNES: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\$(state|derived|effect)\(").unwrap());
    static SVELTE_BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{#\w+").unwrap());
    static VUE_DIR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\bv-if=|\bv-for=|\bv-model=").unwrap());

    if SVELTE_BLOCK.is_match(trimmed) || SVELTE_RUNES.is_match(trimmed) {
        return false;
    }
    if VUE_DIR.is_match(trimmed) {
        return false;
    }

    static DOCTYPE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^<!doctype\s+html").unwrap());
    static HTML_TAG: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^<html[\s>]").unwrap());

    if DOCTYPE.is_match(trimmed) {
        return true;
    }
    if HTML_TAG.is_match(trimmed) {
        return true;
    }

    if !trimmed.starts_with('<') {
        return false;
    }

    let html_tags = [
        "head", "body", "div", "span", "script", "style", "meta", "link", "form", "input",
        "button", "table", "section", "article", "nav", "footer", "header", "main", "aside",
    ];

    let mut match_count = 0;
    for tag in &html_tags {
        let pat = format!("(?i)<{}[\\s>]", regex::escape(tag));
        let re = Regex::new(&pat).unwrap();
        if re.is_match(trimmed) {
            match_count += 1;
        }
    }

    match_count >= 2
}

// ── 3c. XML ──────────────────────────────────────────────────────────────

fn is_likely_xml(trimmed: &str) -> bool {
    if trimmed.starts_with("<?xml") {
        return true;
    }
    if !trimmed.starts_with('<') {
        return false;
    }

    static XMLNS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bxmlns\s*=").unwrap());
    if XMLNS.is_match(trimmed) {
        return true;
    }

    // Leading XML comment — check for tags after it
    if trimmed.starts_with("<!--") {
        static COMMENT_STRIP: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"<!--[\s\S]*?-->\s*").unwrap());
        let after = COMMENT_STRIP.replace_all(trimmed, "");
        let after = after.trim();
        if after.starts_with('<') {
            return true;
        }
    }

    // Opening tag that is NOT a common HTML tag
    static OPEN_TAG: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^<([a-zA-Z_][\w:.\-]*)").unwrap());
    let tag_match = match OPEN_TAG.captures(trimmed) {
        Some(caps) => caps.get(1).unwrap().as_str().to_lowercase(),
        None => return false,
    };

    let html_top_level = [
        "html", "head", "body", "div", "span", "p", "a", "script", "style", "link", "meta",
        "title", "form", "input", "button", "table", "ul", "ol", "li", "h1", "h2", "h3", "h4",
        "h5", "h6", "img", "br", "hr", "section", "article", "nav", "footer", "header", "main",
        "aside", "template",
    ];
    if html_top_level.contains(&tag_match.as_str()) {
        return false;
    }

    // Namespace prefix (e.g. <ns:tag>) → XML
    if tag_match.contains(':') {
        return true;
    }

    // Require both opening and closing tags to confirm structure
    static OPEN_TAGS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"<[a-zA-Z_][\w:.\-]*").unwrap());
    static CLOSE_TAGS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"</[a-zA-Z_][\w:.\-]*").unwrap());

    let open_count = OPEN_TAGS.find_iter(trimmed).count();
    let close_count = CLOSE_TAGS.find_iter(trimmed).count();
    open_count >= 2 && close_count >= 1
}

// ── 3d. Dockerfile ───────────────────────────────────────────────────────

fn is_likely_dockerfile(trimmed: &str) -> bool {
    let lines: Vec<&str> = trimmed
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();

    if lines.is_empty() {
        return false;
    }

    static FIRST_LINE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^(FROM|ARG)\s").unwrap());
    if !FIRST_LINE.is_match(lines[0]) {
        return false;
    }

    static INSTRUCTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(FROM|RUN|CMD|LABEL|MAINTAINER|EXPOSE|ENV|ADD|COPY|ENTRYPOINT|VOLUME|USER|WORKDIR|ARG|ONBUILD|STOPSIGNAL|HEALTHCHECK|SHELL)\s").unwrap()
    });
    let match_count = lines.iter().filter(|l| INSTRUCTION.is_match(l)).count();
    match_count >= 2
}

// ── 3e. CSV / TSV ────────────────────────────────────────────────────────

fn is_likely_csv(trimmed: &str) -> bool {
    let first = trimmed.as_bytes().first().copied().unwrap_or(0);
    if first == b'{' || first == b'[' || first == b'<' {
        return false;
    }

    let lines: Vec<&str> = trimmed
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.len() < 2 {
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
            r"^\s*(#|//|echo|import|from|const|let|var|def|class|function|export)\b",
        )
        .unwrap()
    });
    let script_count = count_matches(&lines, &SCRIPT);
    if script_count as f64 / lines.len() as f64 > 0.3 {
        return false;
    }

    // Curly braces → likely CSS, JS, etc.
    let brace_count = lines
        .iter()
        .filter(|l| l.contains('{') || l.contains('}'))
        .count();
    if brace_count as f64 / lines.len() as f64 > 0.3 {
        return false;
    }

    for delim in &[",", "\t", ";", "|"] {
        if has_consistent_delimiter(&lines, delim) {
            return true;
        }
    }
    false
}

/// Returns true when ≥80% of sampled lines share the same delimiter count
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

// ── 3f. YAML ─────────────────────────────────────────────────────────────

fn is_likely_yaml(trimmed: &str) -> bool {
    let lines: Vec<&str> = trimmed.lines().collect();
    let starts_with_sep = lines.first().map_or(false, |l| l.trim() == "---");

    // Bail out if content looks like Sass/SCSS
    let non_empty: Vec<&str> = lines
        .iter()
        .copied()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();

    static SASS_VAR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*\$[\w\-]+\s*:").unwrap());
    if non_empty.iter().any(|l| SASS_VAR.is_match(l)) {
        return false;
    }

    if non_empty.is_empty() {
        return false;
    }

    static YAML_KV: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*[a-zA-Z_][\w.\-]*\s*:\s").unwrap());
    static YAML_LIST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\-\s+\S").unwrap());
    static CODE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| vec![
        Regex::new(r"^\s*(def|class|if|for|while|return|import|from|try|except|with|async|yield)\s").unwrap(),
        Regex::new(r"^\s*(function|const|let|var|if|for|while|return|import|export|switch|case)\s").unwrap(),
        Regex::new(r"^\s*(#include|int\s+main|typedef|struct)\s").unwrap(),
        Regex::new(r"^\s*(public|private|protected)\s+(class|static|void|int|String)").unwrap(),
        Regex::new(r"^\s*(func|package|type|defer|go)\s").unwrap(),
    ]);

    let mut yaml_lines = 0usize;
    let mut code_lines = 0usize;
    for line in &non_empty {
        if CODE_PATTERNS.iter().any(|p| p.is_match(line)) {
            code_lines += 1;
        } else if YAML_KV.is_match(line) || YAML_LIST.is_match(line) {
            yaml_lines += 1;
        }
    }

    if code_lines > yaml_lines {
        return false;
    }

    // Anti-signal: trailing commas → JS/TS object literal
    static COMMA_TRAILING: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r",\s*(//.*)?$").unwrap());
    let comma_count = non_empty
        .iter()
        .filter(|l| COMMA_TRAILING.is_match(l.trim()))
        .count();
    if comma_count as f64 / non_empty.len() as f64 > 0.3 {
        return false;
    }

    let yaml_ratio = yaml_lines as f64 / non_empty.len() as f64;
    if starts_with_sep && yaml_ratio > 0.3 {
        return true;
    }
    yaml_ratio > 0.5
}

// ── 3g. Markdown ─────────────────────────────────────────────────────────

fn is_likely_markdown(trimmed: &str) -> bool {
    let first = trimmed.as_bytes().first().copied().unwrap_or(0);
    if first == b'<' || first == b'{' || first == b'[' {
        return false;
    }

    // Anti-signals: programming language patterns
    static CODE_ANTI: LazyLock<Vec<(Regex, i32)>> = LazyLock::new(|| vec![
        (Regex::new(r"(?m)^/\*\*?\s").unwrap(), 3),
        (Regex::new(r"(?m)^\s*(import|export)\s+").unwrap(), 3),
        (Regex::new(r"(?m)^\s*(const|let|var)\s+\w+\s*[=:]").unwrap(), 2),
        (Regex::new(r"(?m)^\s*function\s+\w*\s*\(").unwrap(), 2),
        (Regex::new(r"(?m)^\s*(interface|type|enum)\s+\w+").unwrap(), 3),
        (Regex::new(r"(?m)^\s*class\s+\w+").unwrap(), 2),
        (Regex::new(r"(?m)=>\s*[\{(\n]").unwrap(), 2),
        (Regex::new(r"(?m)^\s*def\s+\w+\s*\(").unwrap(), 3),
        (Regex::new(r#"(?m)^\s*#include\s*[<"]"#).unwrap(), 3),
        (Regex::new(r"(?m);\s*$").unwrap(), 1),
        (Regex::new(r"(?m)^\s*async\s+(function|\w+\s*[=(])").unwrap(), 2),
        (Regex::new(r"(?m)^\s*[a-zA-Z_][\w.\-]*\s*:\s+[^h\s]").unwrap(), 4),
    ]);

    let mut code_score = 0i32;
    for (re, weight) in CODE_ANTI.iter() {
        if re.is_match(trimmed) {
            code_score += weight;
        }
    }

    // Strong code signals → not markdown
    if code_score >= 6 {
        return false;
    }

    // Special case: YAML frontmatter → almost certainly markdown
    if has_markdown_frontmatter(trimmed) {
        return true;
    }

    static MD_SIGNALS: LazyLock<Vec<(Regex, i32)>> = LazyLock::new(|| vec![
        (Regex::new(r"(?m)^#{1,6}\s+\S").unwrap(), 3),
        (Regex::new(r"\[.+?\]\(.+?\)").unwrap(), 2),
        (Regex::new(r"!\[.*?\]\(.+?\)").unwrap(), 2),
        (Regex::new(r"(?m)^\s*[\-*+]\s+\S").unwrap(), 1),
        (Regex::new(r"(?m)^\s*\d+\.\s+\S").unwrap(), 1),
        (Regex::new(r"(?m)^\s*>\s+").unwrap(), 1),
        (Regex::new(r"\*\*.+?\*\*").unwrap(), 1),
        (Regex::new(r"(?m)^```").unwrap(), 2),
        (Regex::new(r"(?m)^\|.+\|.+\|").unwrap(), 2),
        (Regex::new(r"(?m)^---\s*$").unwrap(), 1),
    ]);

    let mut score = 0i32;
    for (re, weight) in MD_SIGNALS.iter() {
        if re.is_match(trimmed) {
            score += weight;
        }
    }

    // Moderate code signals → require higher markdown score
    let threshold = if code_score >= 3 { 8 } else { 4 };
    score >= threshold
}

fn has_markdown_frontmatter(trimmed: &str) -> bool {
    if !trimmed.starts_with("---") {
        return false;
    }
    let lines: Vec<&str> = trimmed.lines().collect();
    let close_idx = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| l.trim() == "---")
        .map(|(i, _)| i);
    let close_idx = match close_idx {
        Some(i) if i > 0 => i,
        _ => return false,
    };
    let after_frontmatter: String = lines[close_idx + 1..].join("\n");
    !after_frontmatter.trim().is_empty()
}

// ── 3h. Sass / SCSS ─────────────────────────────────────────────────────

fn detect_sass_scss(trimmed: &str) -> Option<&'static str> {
    let first = trimmed.as_bytes().first().copied().unwrap_or(0);
    if first == b'<' || first == b'{' || first == b'[' {
        return None;
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
        return None;
    }

    // Anchor 1: $variable declarations
    static SASS_VAR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*\$[\w\-]+\s*:\s*.+;?\s*$").unwrap());
    let var_count = count_matches(&lines, &SASS_VAR);

    // Anchor 2: Sass/SCSS-exclusive at-rules
    static SASS_AT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*@(mixin|include|extend|use|forward)\b").unwrap());
    let at_count = count_matches(&lines, &SASS_AT);

    if var_count < 1 && at_count < 1 {
        return None;
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

    if scss_score >= 2 {
        return Some("scss");
    }

    // Indented syntax (no braces) → Sass
    static INDENT_PROP: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s{2,}[a-z\-]+\s*:\s*[^;{}]+\s*$").unwrap());
    let indent_count = count_matches(&lines, &INDENT_PROP);

    static SASS_VAR_NO_SEMI: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*\$[\w\-]+\s*:\s*[^;{}]+\s*$").unwrap());
    let sass_var_count = count_matches(&lines, &SASS_VAR_NO_SEMI);

    if !has_braces && (indent_count >= 1 || sass_var_count >= 2 || at_count >= 1) {
        return Some("sass");
    }

    None
}

// ── 3j. TOML ─────────────────────────────────────────────────────────────

fn is_likely_toml(trimmed: &str) -> bool {
    let first = trimmed.as_bytes().first().copied().unwrap_or(0);
    if first == b'<' {
        return false;
    }
    // Bail for JSON objects `{` or JSON arrays `["..."`
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

    // Bail if Sass/SCSS
    static SASS_VAR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*\$[\w\-]+\s*:").unwrap());
    if non_empty.iter().any(|l| SASS_VAR.is_match(l)) {
        return false;
    }

    let mut score = 0i32;

    // [section] or [[array-of-tables]] headers
    static SECTION: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*\[\[?[\w.\-]+\]\]?\s*$").unwrap());
    let section_count = count_matches(&non_empty, &SECTION);
    if section_count >= 1 {
        score += 2;
    }

    // key = value with TOML-typed values
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

    // YAML uses `:` — if most lines use `:` not `=`, it's likely YAML
    static COLON_KV: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*[\w.\-]+\s*:\s").unwrap());
    let colon_count = count_matches(&non_empty, &COLON_KV);
    if colon_count > kv_count {
        return false;
    }

    score >= 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_simple() {
        assert_eq!(
            detect_structural(r#"{"name": "test", "version": "1.0"}"#, false),
            Some("json")
        );
    }

    #[test]
    fn json_array() {
        assert_eq!(detect_structural("[1, 2, 3]", false), Some("json"));
    }

    #[test]
    fn jsonl() {
        let content = "{\"a\":1}\n{\"b\":2}\n{\"c\":3}";
        assert_eq!(detect_structural(content, false), Some("json"));
    }

    #[test]
    fn html_doctype() {
        assert_eq!(
            detect_structural("<!DOCTYPE html>\n<html><head></head></html>", false),
            Some("html")
        );
    }

    #[test]
    fn xml_processing_instruction() {
        assert_eq!(
            detect_structural("<?xml version=\"1.0\"?>\n<root/>", false),
            Some("xml")
        );
    }

    #[test]
    fn dockerfile_basic() {
        let content = "FROM python:3.11\nRUN pip install flask\nCOPY . /app\nCMD [\"python\", \"app.py\"]";
        assert_eq!(detect_structural(content, false), Some("dockerfile"));
    }

    #[test]
    fn csv_comma() {
        let content = "name,age,city\nAlice,30,NYC\nBob,25,LA\nCharlie,35,Chicago";
        assert_eq!(detect_structural(content, false), Some("csv"));
    }

    #[test]
    fn csv_tab() {
        let content = "name\tage\tcity\nAlice\t30\tNYC\nBob\t25\tLA";
        assert_eq!(detect_structural(content, false), Some("csv"));
    }

    #[test]
    fn markdown_headings_and_links() {
        let content = "# My Document\n\nSome text with a [link](http://example.com).\n\n## Section\n\n- Item 1\n- Item 2";
        assert_eq!(detect_structural(content, false), Some("markdown"));
    }

    #[test]
    fn markdown_frontmatter() {
        let content = "---\ntitle: My Post\ndate: 2024-01-01\n---\n\n# Hello World\n\nSome content.";
        assert_eq!(detect_structural(content, false), Some("markdown"));
    }

    #[test]
    fn yaml_basic() {
        let content = "name: my-app\nversion: 1.0.0\ndependencies:\n  - flask\n  - gunicorn";
        assert_eq!(detect_structural(content, false), Some("yaml"));
    }

    #[test]
    fn yaml_with_separator() {
        let content = "---\napiVersion: v1\nkind: Service\nmetadata:\n  name: my-service";
        // This looks like yaml with --- prefix and enough key: value lines
        // Note: markdown frontmatter has content after ---...--- block
        // This is just --- at start with yaml-like lines → yaml
        assert_eq!(detect_structural(content, false), Some("yaml"));
    }

    #[test]
    fn toml_basic() {
        let content = "[package]\nname = \"my-app\"\nversion = \"0.1.0\"\nedition = \"2021\"";
        assert_eq!(detect_structural(content, false), Some("toml"));
    }

    #[test]
    fn php_open_tag() {
        assert_eq!(
            detect_structural("<?php\necho 'hello';", false),
            Some("php")
        );
    }

    #[test]
    fn svelte_runes() {
        let content = "<script lang=\"ts\">\nlet count = $state(0);\nlet doubled = $derived(count * 2);\n</script>\n<button on:click={() => count++}>{count}</button>";
        assert_eq!(detect_structural(content, false), Some("svelte"));
    }

    #[test]
    fn vue_template() {
        let content = "<template>\n  <div v-if=\"show\">\n    <button @click=\"toggle\">Toggle</button>\n  </div>\n</template>\n<script setup>\nconst show = ref(true);\n</script>";
        assert_eq!(detect_structural(content, false), Some("vue"));
    }

    #[test]
    fn scss_with_variables() {
        let content = "$primary: #333;\n$font-size: 16px;\n\nbody {\n  color: $primary;\n  font-size: $font-size;\n}";
        assert_eq!(detect_structural(content, false), Some("scss"));
    }

    #[test]
    fn not_csv_yaml() {
        // YAML key: value lines should not be detected as CSV
        let content = "name: test\nversion: 1.0\nauthor: someone\nlicense: MIT";
        assert_ne!(detect_structural(content, false), Some("csv"));
    }
}
