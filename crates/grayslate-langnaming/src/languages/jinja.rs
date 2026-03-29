use super::{NamingDefinition, Extractor};
use crate::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "jinja",
        extension: "j2",
        extract: Extractor::Custom(extract_jinja),
    }
}

/// Jinja/Jinja2 template naming: {% extends %}, {% block %} names.
fn extract_jinja(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static EXTENDS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"\{%[-\s]+extends\s+["']([^"']+)["']"#).unwrap()
    });
    static BLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{%[-\s]+block\s+(\w+)").unwrap()
    });
    static MACRO_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{%[-\s]+macro\s+(\w+)").unwrap()
    });

    let mut tokens: Vec<String> = Vec::new();

    // {% extends "base.html" %} — the template this extends
    if let Some(cap) = EXTENDS_RE.captures(content) {
        let parent = &cap[1];
        // Strip extension: base.html → base
        let stem = parent.rsplit('/').next().unwrap_or(parent);
        let stem = stem.strip_suffix(".html").or_else(|| stem.strip_suffix(".j2"))
            .or_else(|| stem.strip_suffix(".jinja2")).unwrap_or(stem);
        tokens.push(format!("extends-{stem}"));
    }

    // {% macro render_item() %}
    for cap in MACRO_RE.captures_iter(content).take(2) {
        if tokens.len() >= MAX_TOKENS { break; }
        tokens.push(cap[1].to_string());
    }

    // {% block content %}
    for cap in BLOCK_RE.captures_iter(content).take(3) {
        if tokens.len() >= MAX_TOKENS { break; }
        let name = &cap[1];
        if !matches!(name, "content" | "body" | "head" | "title" | "scripts" | "styles") {
            tokens.push(name.to_string());
        }
    }

    tokens.truncate(MAX_TOKENS);
    if tokens.is_empty() {
        // Fall back to HTML extraction if the template has HTML content
        crate::markup::extract_xml_html(content)
    } else {
        Some(tokens.join("-"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_jinja(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn extends_and_blocks() {
        let src = "{% extends \"layouts/base.html\" %}\n{% block sidebar %}\n  <nav>...</nav>\n{% endblock %}\n{% block main_content %}\n  <div>Hello</div>\n{% endblock %}";
        let n = name(src).unwrap();
        assert!(n.contains("extends-base"), "got: {n}");
    }

    #[test]
    fn macro_template() {
        let src = "{% macro render_field(field) %}\n  <div>{{ field.label }}</div>\n{% endmacro %}";
        let n = name(src).unwrap();
        assert!(n.contains("render-field"), "got: {n}");
    }

    #[test]
    fn block_only_custom_name() {
        let src = "{% block navigation %}\n  <nav><a href=\"/\">Home</a></nav>\n{% endblock %}\n{% block footer %}\n  <footer>2026</footer>\n{% endblock %}";
        let n = name(src).unwrap();
        assert!(n.contains("navigation"), "non-noise block: {n}");
    }

    #[test]
    fn html_fallback_no_jinja() {
        let src = "<div id=\"promo-banner\">\n  <h2>Sale!</h2>\n</div>";
        let n = name(src).unwrap();
        assert!(n.contains("promo-banner"), "HTML fallback: {n}");
    }
}
