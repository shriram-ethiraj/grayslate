use regex::Regex;
use std::sync::LazyLock;

/// Markdown-heading attribute blocks: `{ #anchor }`, `{#anchor}`, `{ .class #id }`, `{: #id }`.
/// These are Pandoc/MkDocs/Kramdown processor directives — never literal heading text.
static HEADING_ATTRS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\{[^}]*\}").unwrap());

/// Inline HTML tags like `<abbr title="...">`, `</abbr>`, `<strong>`.
static HTML_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

/// XML / HTML: root element name + up to two attribute values.
pub(crate) fn extract_xml_html(content: &str) -> Option<String> {
    // Find the first tag: <tagname attr="val" ...>
    // Skip XML declaration and DOCTYPE.
    let tag_re = regex::Regex::new(r"<([a-zA-Z][a-zA-Z0-9\-:_]*)").ok()?;
    let attr_re =
        regex::Regex::new(r#"(?:id|name|class|title)\s*=\s*["']([^"']{1,40})["']"#).ok()?;

    let trimmed = content.trim();
    let mut tokens: Vec<String> = Vec::new();

    for cap in tag_re.captures_iter(trimmed).take(8) {
        let name = &cap[1];
        // Skip XML declaration and meta tags.
        if name.eq_ignore_ascii_case("?xml")
            || name.eq_ignore_ascii_case("!DOCTYPE")
            || name.eq_ignore_ascii_case("meta")
            || name.eq_ignore_ascii_case("link")
            || name.eq_ignore_ascii_case("script")
            || name.eq_ignore_ascii_case("style")
        {
            continue;
        }
        tokens.push(name.to_string());
        break;
    }

    // Add meaningful attribute values.
    for cap in attr_re.captures_iter(trimmed).take(2) {
        let val = cap[1].trim();
        if !val.is_empty() {
            tokens.push(val.to_string());
        }
    }

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join("-"))
    }
}

/// Markdown: first `# heading` or frontmatter `title:` value.
pub(crate) fn extract_markdown(content: &str) -> Option<String> {
    let mut in_frontmatter = false;
    let mut frontmatter_done = false;
    let mut first_line_was_dashes = false;

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // YAML frontmatter block.
        if i == 0 && trimmed == "---" {
            in_frontmatter = true;
            first_line_was_dashes = true;
            continue;
        }
        if in_frontmatter {
            if trimmed == "---" || trimmed == "..." {
                in_frontmatter = false;
                frontmatter_done = true;
                continue;
            }
            // title: My Document
            if let Some(rest) = trimmed.strip_prefix("title:") {
                let title = rest.trim().trim_matches('"').trim_matches('\'').trim();
                if !title.is_empty() {
                    return Some(title.to_string());
                }
            }
            continue;
        }
        let _ = (frontmatter_done, first_line_was_dashes); // suppress warnings

        // ATX headings: # H1, ## H2, etc.
        if trimmed.starts_with('#') {
            let raw = trimmed.trim_start_matches('#').trim();
            if raw.is_empty() {
                continue;
            }
            // Skip copyright/license headings — they're boilerplate, not content.
            let lower = raw.to_lowercase();
            if lower.starts_with("copyright")
                || lower.starts_with("license")
                || lower.starts_with("all rights reserved")
            {
                continue;
            }
            // Strip inline HTML tags (e.g. <abbr title="...">text</abbr>), keeping inner text.
            let cleaned = HTML_TAG_RE.replace_all(raw, "");
            // Strip inline backtick code markers (e.g. `Request` → Request).
            let cleaned = cleaned.replace('`', "");
            // Strip Pandoc/MkDocs/Kramdown attribute blocks (e.g. { #anchor }, {#id}, {: .class }).
            let cleaned = HEADING_ATTRS_RE.replace_all(cleaned.trim(), "");
            let heading = cleaned.trim();
            if !heading.is_empty() {
                return Some(heading.to_string());
            }
            // Heading was entirely attributes/markup — keep scanning for a real one.
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::shared::slugify;

    fn md(name: &str) -> Option<String> {
        extract_markdown(name).and_then(|s| slugify(&s))
    }

    // --- ATX heading: plain text ---
    #[test]
    fn plain_heading() {
        assert_eq!(md("# Getting Started"), Some("getting-started".into()));
    }

    // --- ATX heading: MkDocs Material { #anchor } (with spaces) ---
    #[test]
    fn mkdocs_anchor_with_spaces() {
        assert_eq!(md("# FastAPI { #fastapi }"), Some("fast-api".into()));
    }

    // --- ATX heading: Pandoc compact {#anchor} ---
    #[test]
    fn pandoc_anchor_compact() {
        assert_eq!(md("# Security Policy {#security-policy}"), Some("security-policy".into()));
    }

    // --- ATX heading: class + anchor { .class #id } ---
    #[test]
    fn mkdocs_class_and_anchor() {
        assert_eq!(md("# Async { .tip #async }"), Some("async".into()));
    }

    // --- ATX heading: Kramdown {: #id .class} ---
    #[test]
    fn kramdown_anchor() {
        assert_eq!(md("# Introduction {: #intro .lead}"), Some("introduction".into()));
    }

    // --- ATX heading: backtick inline code ---
    #[test]
    fn backtick_inline_code() {
        assert_eq!(md("# `Request` class"), Some("request-class".into()));
    }

    // --- ATX heading: backtick + anchor ---
    #[test]
    fn backtick_and_anchor() {
        assert_eq!(
            md("# Dependencies with `yield` { #dependencies-with-yield }"),
            Some("dependencies-with-yield".into()),
        );
    }

    // --- ATX heading: inline HTML <abbr> kept as inner text ---
    #[test]
    fn inline_html_abbr() {
        // Tags are stripped; inner text is kept.
        assert_eq!(
            md(r#"# <abbr title="En anglais: Debugging">Débogage</abbr> { #debugging }"#),
            Some("débogage".into()),
        );
    }

    // --- Heading that is entirely markup → skip, fall through to next ---
    #[test]
    fn heading_entirely_stripped_falls_through() {
        // First heading becomes empty after stripping; second heading is used.
        let input = "# { #only-anchor }\n## Real Title";
        assert_eq!(md(input), Some("real-title".into()));
    }

    // --- Frontmatter title: unaffected by heading cleanup ---
    #[test]
    fn frontmatter_title_unaffected() {
        let input = "---\ntitle: My Document\n---\n# Should be ignored";
        assert_eq!(md(input), Some("my-document".into()));
    }

    // --- H2 heading used when H1 is absent ---
    #[test]
    fn h2_heading_used() {
        assert_eq!(md("Some text\n\n## Overview"), Some("overview".into()));
    }

    // --- Empty content → None ---
    #[test]
    fn empty_content() {
        assert_eq!(md(""), None);
    }
}
