use super::{wp, LanguageDefinition};
use super::ContentFamily;
use regex::Regex;
use std::sync::LazyLock;

pub(crate) fn is_likely_html(trimmed: &str, _was_sliced: bool) -> bool {
    if trimmed.starts_with("<?xml") {
        return false;
    }

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

    static HTML_BLOCK_TAGS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        ["head", "body", "div", "span", "script", "style", "meta", "link", "form", "input",
         "button", "table", "section", "article", "nav", "footer", "header", "main", "aside"]
            .iter()
            .map(|t| Regex::new(&format!("(?i)<{}[\\s>]", t)).unwrap())
            .collect()
    });

    HTML_BLOCK_TAGS.iter().filter(|re| re.is_match(trimmed)).count() >= 2
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "html",
        extensions: &[".html", ".htm", ".xhtml"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(40),
        structural_detect: Some(is_likely_html),
        keywords: &[
            "DOCTYPE", "html", "head", "body", "div", "span", "script", "style",
            "link", "meta", "title", "form", "input", "button", "table", "thead",
            "tbody", "section", "article", "header", "footer", "nav", "main", "aside",
        ],
        builtins: &[],
        // ── Family-gated fields ───────────────────────────────
        content_families: &[ContentFamily::Markup],
        anchors: &[
            wp!(r"(?i)<!doctype\s+html", 5),
            wp!(r"<html[\s>]", 4),
            // HTML-specific meta/link tags
            wp!(r"<meta\s+charset=", 4),
            wp!(r"<link\s+rel=\x22stylesheet\x22", 4),
        ],
        hints: &[
            wp!(r"<head[\s>]", 3),
            wp!(r"<body[\s>]", 3),
            wp!(r"<(div|span|p|a|img|form|input|button|table|ul|ol|li|h[1-6])[\s>/]", 2),
            wp!(r"\b(class|id|href|src|alt|style|type|name|value)=", 2),
        ],
        disqualifiers: &[
            // Svelte block syntax — means Svelte, not plain HTML
            wp!(r"\{#(if|each|await)\s", -5),
            // Vue directives — means Vue, not plain HTML
            wp!(r"\bv-(if|for|bind|on|model)\b", -5),
        ],
    }
}
