use super::LanguageDefinition;
use regex::Regex;
use std::sync::LazyLock;

/// Strip fenced code blocks (` ``` `...` ``` `) and indented code blocks (4+ spaces
/// after a blank line) so embedded code snippets don't trigger anti-signals
/// against markdown detection or heuristic language scoring.
///
/// Keeps fence markers themselves (they're a markdown signal).
pub(crate) fn strip_code_blocks(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut in_fence = false;
    let mut prev_blank = false;
    let mut in_indent_block = false;

    for line in content.lines() {
        let trimmed_line = line.trim();

        if trimmed_line.starts_with("```") {
            in_fence = !in_fence;
            in_indent_block = false;
            result.push_str(line);
            result.push('\n');
            prev_blank = false;
            continue;
        }

        if in_fence {
            prev_blank = false;
            continue;
        }

        let is_indented = line.starts_with("    ") || line.starts_with('\t');
        if trimmed_line.is_empty() {
            if in_indent_block {
                prev_blank = true;
                continue;
            }
            prev_blank = true;
            result.push('\n');
            continue;
        }

        if is_indented && (prev_blank || in_indent_block) {
            in_indent_block = true;
            prev_blank = false;
            continue;
        }

        in_indent_block = false;
        prev_blank = false;
        result.push_str(line);
        result.push('\n');
    }
    result
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
    let after = after_frontmatter.trim();

    if after.is_empty() {
        return true;
    }

    static COMPONENT_GUARD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)(^\s*<(script|style|Layout|Component|Fragment)\b|^\s*import\s+[\w\{].*from\s+['"]|^\s*export\s+(const|default|function|let)\s|<[A-Z]\w+[\s/>])"#).unwrap()
    });
    if COMPONENT_GUARD.is_match(after) {
        return false;
    }

    true
}

pub(crate) fn is_likely_markdown(trimmed: &str, _was_sliced: bool) -> bool {
    let first = trimmed.as_bytes().first().copied().unwrap_or(0);
    if first == b'<' || first == b'{' || first == b'[' {
        return false;
    }

    if has_markdown_frontmatter(trimmed) {
        return true;
    }

    static MDX_HEADING: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^#{1,6}\s+\S").unwrap());
    static MDX_IMPORT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?m)^\s*import\s+[\w\{*].*\s+from\s+['"`]"#).unwrap());
    if MDX_HEADING.is_match(trimmed) && MDX_IMPORT.is_match(trimmed) {
        return true;
    }

    let prose_only = strip_code_blocks(trimmed);

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
        if re.is_match(&prose_only) {
            code_score += weight;
        }
    }

    if code_score >= 6 {
        return false;
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

    let threshold = if code_score >= 3 { 8 } else { 3 };
    score >= threshold
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "markdown",
        extensions: &[".md", ".markdown", ".mdx"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(80),
        structural_detect: Some(is_likely_markdown),
        patterns: &[],
        anti_patterns: &[],
        uses_hash_comments: false,
        keywords: &[],
        builtins: &[],
        illegal: None,
        extends: None,
    }
}
