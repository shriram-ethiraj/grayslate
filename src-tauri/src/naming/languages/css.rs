use std::collections::HashSet;

use super::NamingDefinition;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "css",
        extension: "css",
        extract: extract_css,
    }
}

/// CSS naming: extract prominent selectors, @keyframes, and framework markers.
fn extract_css(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    // Framework/library detection
    static NORMALIZE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)normalize\.css|normalize\s").unwrap()
    });
    static RESET_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(?:css\s*reset|reset\.css|meyer.*reset)").unwrap()
    });
    static TAILWIND_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"@tailwind\s").unwrap()
    });

    // Comment-based naming: /* Theme: ... */ or /* File: ... */
    static COMMENT_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)/\*[\s*]*(?:Theme|File|Module|Component|Section)\s*:\s*(.{3,50}?)\s*\*/"#).unwrap()
    });

    // @keyframes, @charset, @namespace
    static KEYFRAMES_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)@keyframes\s+([\w-]+)").unwrap()
    });

    // Prominent selectors: IDs and class selectors (first-of-kind)
    static ID_SELECTOR_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^#([\w-]+)\s*\{").unwrap()
    });
    static CLASS_SELECTOR_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\.([\w-]+)\s*\{").unwrap()
    });

    // Framework detection
    if NORMALIZE_RE.is_match(content) { return Some("normalize".to_string()); }
    if RESET_RE.is_match(content) { return Some("css-reset".to_string()); }
    if TAILWIND_RE.is_match(content) { return Some("tailwind-base".to_string()); }

    // Comment-based name
    if let Some(cap) = COMMENT_NAME_RE.captures(content) {
        return Some(cap[1].trim().to_string());
    }

    let mut tokens: Vec<String> = Vec::new();
    let mut seen = HashSet::new();

    // ID selectors (P10)
    for cap in ID_SELECTOR_RE.captures_iter(content).take(3) {
        let name = cap[1].to_string();
        if seen.insert(name.clone()) && !is_css_noise(&name) {
            tokens.push(name);
        }
    }

    // @keyframes (P8)
    for cap in KEYFRAMES_RE.captures_iter(content).take(2) {
        let name = cap[1].to_string();
        if seen.insert(name.clone()) {
            tokens.push(name);
        }
    }

    // Class selectors (P7)
    for cap in CLASS_SELECTOR_RE.captures_iter(content).take(4) {
        if tokens.len() >= MAX_TOKENS { break; }
        let name = cap[1].to_string();
        if seen.insert(name.clone()) && !is_css_noise(&name) {
            tokens.push(name);
        }
    }

    tokens.truncate(MAX_TOKENS);
    if tokens.is_empty() { None } else { Some(tokens.join("-")) }
}

fn is_css_noise(name: &str) -> bool {
    matches!(name, "root" | "body" | "html" | "main" | "wrapper" | "container"
        | "content" | "page" | "app" | "site" | "header" | "footer" | "nav"
        | "sidebar" | "section" | "article")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_css(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn keyframes_and_selector() {
        let src = "@keyframes fadeIn {\n  from { opacity: 0; }\n  to { opacity: 1; }\n}\n.card {\n  animation: fadeIn 0.3s;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("fade-in"), "got: {n}");
    }

    #[test]
    fn id_selector() {
        let src = "#dashboard {\n  display: grid;\n}\n.widget {\n  padding: 1rem;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("dashboard"), "got: {n}");
    }

    #[test]
    fn normalize_detection() {
        let src = "/*! normalize.css v8.0.1 | MIT License */\nhtml { line-height: 1.15; }";
        let n = name(src).unwrap();
        assert_eq!(n, "normalize");
    }

    #[test]
    fn comment_theme() {
        let src = "/* Theme: Dark Mode */\n:root { --bg: #1a1a1a; }";
        let n = name(src).unwrap();
        assert!(n.contains("dark-mode"), "got: {n}");
    }
}
