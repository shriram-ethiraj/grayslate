use super::{wp, LanguageDefinition};
use super::ContentFamily;
use regex::Regex;
use std::sync::LazyLock;

pub(crate) fn is_likely_xml(trimmed: &str, _was_sliced: bool) -> bool {
    if trimmed.starts_with("<?xml") {
        return true;
    }
    if !trimmed.starts_with('<') {
        return false;
    }

    static PHP_TAG: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"<\?(php|=)\b").unwrap());
    if PHP_TAG.is_match(trimmed) {
        return false;
    }

    static XMLNS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bxmlns\s*=").unwrap());
    if XMLNS.is_match(trimmed) {
        return true;
    }

    if trimmed.starts_with("<!--") {
        static COMMENT_STRIP: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"<!--[\s\S]*?-->\s*").unwrap());
        let after = COMMENT_STRIP.replace_all(trimmed, "");
        let after = after.trim();
        if after.starts_with('<') {
            return true;
        }
    }

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

    if tag_match.contains(':') {
        return true;
    }

    static OPEN_TAGS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"<[a-zA-Z_][\w:.\-]*").unwrap());
    static CLOSE_TAGS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"</[a-zA-Z_][\w:.\-]*").unwrap());

    let open_count = OPEN_TAGS.find_iter(trimmed).count();
    let close_count = CLOSE_TAGS.find_iter(trimmed).count();
    open_count >= 2 && close_count >= 1
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "xml",
        extensions: &[".xml", ".svg", ".plist", ".xsl", ".xslt", ".xsd", ".wsdl", ".rss", ".atom", ".xaml", ".csproj", ".fsproj", ".vcxproj"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(50),
        structural_detect: Some(is_likely_xml),
        keywords: &[],
        builtins: &[],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Markup],
        anchors: &[
            // <?xml version= — XML declaration
            wp!(r"<\?xml\s+version=", 5),
            // xmlns= — XML namespace declaration
            wp!(r"\bxmlns\s*=", 5),
            // <![CDATA[ — CDATA section
            wp!(r"<!\[CDATA\[", 5),
            // Namespace-prefixed tags: <foo:bar>
            wp!(r"<\w+:\w+[\s>]", 4),
            // <!ENTITY — XML entity declaration (in DTD)
            wp!(r"<!ENTITY\s+", 4),
        ],
        hints: &[
            // Self-closing tags with attributes
            wp!(r"<\w+\s+[^>]+/>", 2),
            // <!ENTITY — XML entity declarations
            wp!(r"<!ENTITY\s+", 3),
            // Processing instructions <?target ... ?>
            wp!(r"<\?\w+\s+", 3),
            // XML comments
            wp!(r"<!--", 2),
            // Closing namespace-prefixed tags
            wp!(r"</\w+:\w+>", 2),
        ],
        disqualifiers: &[
            // <!DOCTYPE html> — means HTML, not XML
            wp!(r"(?i)<!DOCTYPE\s+html", -5),
        ],
    }
}
