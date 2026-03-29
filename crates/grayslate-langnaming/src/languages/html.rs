use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "html",
        extension: "html",
        extract: Extractor::Custom(extract_html),
    }
}

/// HTML naming: extract <title> content first, then <h1>, then metadata, then fall back to shared.
fn extract_html(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    // <title>Page Title</title>
    static TITLE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)<title[^>]*>\s*([^<]{1,80})\s*</title>").unwrap()
    });
    // <h1>Heading</h1> or <h1 class="...">Heading</h1>
    static H1_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)<h1[^>]*>\s*([^<]{1,80})\s*</h1>").unwrap()
    });
    // <meta name="description" content="...">
    static META_DESC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)<meta\s+name\s*=\s*["']description["']\s+content\s*=\s*["']([^"']{1,80})["']"#).unwrap()
    });
    // Open Graph: <meta property="og:title" content="...">
    static OG_TITLE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)<meta\s+property\s*=\s*["']og:title["']\s+content\s*=\s*["']([^"']{1,80})["']"#).unwrap()
    });
    // Open Graph description
    static OG_DESC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)<meta\s+property\s*=\s*["']og:description["']\s+content\s*=\s*["']([^"']{1,80})["']"#).unwrap()
    });
    // Twitter Card: <meta name="twitter:title" content="...">
    static TWITTER_TITLE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)<meta\s+name\s*=\s*["']twitter:title["']\s+content\s*=\s*["']([^"']{1,80})["']"#).unwrap()
    });

    if let Some(cap) = TITLE_RE.captures(content) {
        let title = cap[1].trim();
        if !title.is_empty() {
            return Some(title.to_string());
        }
    }

    if let Some(cap) = H1_RE.captures(content) {
        let heading = cap[1].trim();
        if !heading.is_empty() {
            return Some(heading.to_string());
        }
    }

    // Open Graph / Twitter metadata before generic meta description
    if let Some(cap) = OG_TITLE_RE.captures(content) {
        let title = cap[1].trim();
        if !title.is_empty() {
            return Some(title.to_string());
        }
    }

    if let Some(cap) = TWITTER_TITLE_RE.captures(content) {
        let title = cap[1].trim();
        if !title.is_empty() {
            return Some(title.to_string());
        }
    }

    if let Some(cap) = META_DESC_RE.captures(content) {
        let desc = cap[1].trim();
        if !desc.is_empty() {
            return Some(desc.to_string());
        }
    }

    if let Some(cap) = OG_DESC_RE.captures(content) {
        let desc = cap[1].trim();
        if !desc.is_empty() {
            return Some(desc.to_string());
        }
    }

    // Fall back to shared XML/HTML extractor (root element + attributes)
    crate::markup::extract_xml_html(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_html(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn title_tag() {
        let src = "<!DOCTYPE html>\n<html>\n<head><title>My Dashboard</title></head>\n<body></body></html>";
        let n = name(src).unwrap();
        assert!(n.contains("my-dashboard"), "got: {n}");
    }

    #[test]
    fn h1_fallback() {
        let src = "<html><body><h1>Welcome to the App</h1></body></html>";
        let n = name(src).unwrap();
        assert!(n.contains("welcome"), "got: {n}");
    }

    #[test]
    fn meta_description() {
        let src = r#"<html><head><meta name="description" content="User management portal"></head></html>"#;
        let n = name(src).unwrap();
        assert!(n.contains("user-management"), "got: {n}");
    }

    #[test]
    fn og_title() {
        let src = r#"<html><head><meta property="og:title" content="Product Launch 2026"></head></html>"#;
        let n = name(src).unwrap();
        assert!(n.contains("product-launch"), "got: {n}");
    }

    #[test]
    fn twitter_title() {
        let src = r#"<html><head><meta name="twitter:title" content="Developer Blog Post"></head></html>"#;
        let n = name(src).unwrap();
        assert!(n.contains("developer-blog"), "got: {n}");
    }

    #[test]
    fn og_description_fallback() {
        let src = r#"<html><head><meta property="og:description" content="A guide to Rust macros"></head><body></body></html>"#;
        let n = name(src).unwrap();
        assert!(n.contains("guide-to-rust"), "og:description fallback: {n}");
    }

    #[test]
    fn root_element_fallback() {
        let src = r#"<div id="settings-panel"><p>Settings go here</p></div>"#;
        let n = name(src).unwrap();
        assert!(n.contains("settings-panel"), "root element + id: {n}");
    }
}
