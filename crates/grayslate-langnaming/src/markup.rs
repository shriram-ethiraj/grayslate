use regex::Regex;
use std::sync::LazyLock;

/// Markdown-heading attribute blocks: `{ #anchor }`, `{#anchor}`, `{ .class #id }`, `{: #id }`.
/// These are Pandoc/MkDocs/Kramdown processor directives — never literal heading text.
static HEADING_ATTRS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\{[^}]*\}").unwrap());

/// Inline HTML tags like `<abbr title="...">`, `</abbr>`, `<strong>`.
static HTML_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

/// Markdown inline link: `[text](url)` → keep only `text`.
static MD_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\([^)]*\)").unwrap());

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

/// Markdown: first `# heading` or frontmatter `title:` / `name:` value.
///
/// Skips version-only headings (e.g. `# 3.4.1`) and detects known document
/// types (changelog, readme, contributing, etc.) when all headings are weak.
pub(crate) fn extract_markdown(content: &str) -> Option<String> {
    // Version-only heading: digits, dots, optional v prefix, optional -beta/rc suffix
    static VERSION_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^v?\d+(?:\.\d+)*(?:[-.]?(?:alpha|beta|rc|dev|snapshot|pre|post|RELEASE|FINAL|GA)\d*)?$").unwrap());

    let mut in_frontmatter = false;
    let mut frontmatter_done = false;
    let mut first_line_was_dashes = false;
    let mut frontmatter_name: Option<String> = None;

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
            // title: My Document — highest priority
            if let Some(rest) = trimmed.strip_prefix("title:") {
                let title = rest.trim().trim_matches('"').trim_matches('\'').trim();
                if !title.is_empty() {
                    return Some(title.to_string());
                }
            }
            // name: Bug Report — fallback when title is empty (issue templates)
            if let Some(rest) = trimmed.strip_prefix("name:") {
                let name = rest.trim().trim_matches('"').trim_matches('\'').trim();
                if !name.is_empty() && frontmatter_name.is_none() {
                    frontmatter_name = Some(name.to_string());
                }
            }
            continue;
        }
        let _ = (frontmatter_done, first_line_was_dashes); // suppress warnings

        // If frontmatter had name: but no title:, use it now
        if frontmatter_done {
            if let Some(ref name) = frontmatter_name {
                return Some(name.clone());
            }
            // Reset so we only check once
            frontmatter_done = false;
        }

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
            // Strip markdown links (e.g. [1.9.0](https://...)) → keep link text only.
            let cleaned = MD_LINK_RE.replace_all(&cleaned, "$1");
            // Strip inline backtick code markers (e.g. `Request` → Request).
            let cleaned = cleaned.replace('`', "");
            // Strip Pandoc/MkDocs/Kramdown attribute blocks (e.g. { #anchor }, {#id}, {: .class }).
            let cleaned = HEADING_ATTRS_RE.replace_all(cleaned.trim(), "");
            let heading = cleaned.trim();
            if heading.is_empty() {
                continue;
            }
            // Skip version-only headings (e.g. "3.4.1", "v2.0.0-beta1")
            if VERSION_RE.is_match(heading) {
                continue;
            }
            return Some(heading.to_string());
        }
    }

    // If frontmatter name: was found but nothing else, use it
    if let Some(name) = frontmatter_name {
        return Some(name);
    }

    // Fallback: detect known document types by content patterns
    detect_document_type(content)
}

/// Detect well-known document types when headings fail.
fn detect_document_type(content: &str) -> Option<String> {
    let lower = content.to_lowercase();
    let sample = if lower.len() > 2000 { &lower[..2000] } else { &lower };

    // Changelog: version headers, "## [X.Y.Z]", "### Added/Changed/Fixed"
    if sample.contains("changelog")
        || (sample.contains("## [") && (sample.contains("### added") || sample.contains("### changed") || sample.contains("### fixed")))
        || (sample.contains("## [") && sample.contains("](http"))
    {
        return Some("changelog".to_string());
    }

    // Contributing guide
    if sample.contains("contributing")
        && (sample.contains("pull request") || sample.contains("issue") || sample.contains("fork"))
    {
        return Some("contributing-guide".to_string());
    }

    // Security policy
    if sample.contains("security") && sample.contains("vulnerabilit") {
        return Some("security-policy".to_string());
    }

    // Code of conduct
    if sample.contains("code of conduct") || sample.contains("contributor covenant") {
        return Some("code-of-conduct".to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::slugify;

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

    // --- New: version-only headings skipped ---
    #[test]
    fn version_only_heading_skipped() {
        let input = "# 3.4.1\n\nSome release notes content.";
        // Version heading skipped, no other heading → None or doc-type fallback
        assert_eq!(md(input), None);
    }

    #[test]
    fn version_heading_falls_through_to_next() {
        let input = "# 3.4.1\n## Improvements\n\n- Added feature X";
        assert_eq!(md(input), Some("improvements".into()));
    }

    #[test]
    fn version_with_v_prefix_skipped() {
        let input = "# v2.0.0-beta1\n## Breaking Changes\n\n- Removed old API";
        assert_eq!(md(input), Some("breaking-changes".into()));
    }

    // --- New: frontmatter name: when title: is empty ---
    #[test]
    fn frontmatter_name_when_no_title() {
        let input = "---\nname: Bug Report\nabout: Create a report to help us improve\ntitle: ''\nlabels: bug\n---\n\n## Description";
        assert_eq!(md(input), Some("bug-report".into()));
    }

    #[test]
    fn frontmatter_title_still_preferred() {
        let input = "---\nname: Bug Report\ntitle: Report a Bug\n---\n\n## Description";
        assert_eq!(md(input), Some("report-a-bug".into()));
    }

    // --- New: document-type detection fallback ---
    #[test]
    fn changelog_detection() {
        // All headings are version-only → falls through to detect_document_type
        let input = "# 2.0.0\n\n## [1.9.0](https://github.com/...)\n\n- New feature\n- Updated API\n\n## [1.8.0](https://github.com/...)";
        assert_eq!(md(input), Some("changelog".into()));
    }

    #[test]
    fn contributing_guide_detection() {
        let input = "Thank you for contributing to this project.\n\nPlease submit a pull request with your changes.\nCreate an issue first to discuss the feature or fork the repo.";
        assert_eq!(md(input), Some("contributing-guide".into()));
    }

    #[test]
    fn security_policy_detection() {
        let input = "# Security\n\nReport security vulnerabilities to security@example.com";
        // Has a heading "Security" so it returns that directly
        assert_eq!(md(input), Some("security".into()));
    }

    #[test]
    fn markdown_link_in_heading() {
        let input = "# [Installation Guide](https://docs.example.com/install)\n\nFollow steps below.";
        assert_eq!(md(input), Some("installation-guide".into()));
    }

    #[test]
    fn code_of_conduct_detection() {
        let input = "This project adheres to a code of conduct.\nAll participants are expected to be respectful.";
        assert_eq!(md(input), Some("code-of-conduct".into()));
    }
}
