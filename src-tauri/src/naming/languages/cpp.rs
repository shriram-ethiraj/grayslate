use std::collections::HashSet;

use super::NamingDefinition;
use crate::naming::code::{extract_with_tree_sitter, pick_c_or_cpp_grammar};
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "cpp",
        extension: "cpp",
        extract: extract_cpp,
    }
}

/// C++ naming: tree-sitter first, then regex fallback for templates/macros.
fn extract_cpp(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    // Try tree-sitter first (reuse C's shared collector)
    if let Some(result) = extract_with_tree_sitter(
        content,
        pick_c_or_cpp_grammar(content),
        super::c::collect_c_cpp,
    ) {
        return Some(result);
    }

    // Regex fallback: namespace, class, template, #define
    static NAMESPACE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^namespace\s+([a-zA-Z_]\w*)").unwrap());
    static CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:template\s*<[^>]*>\s*)?(?:class|struct)\s+([A-Z][a-zA-Z0-9_]*)")
            .unwrap()
    });
    static HEADER_GUARD_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^#ifndef\s+([A-Z][A-Z0-9_]+_H(?:PP|XX)?)").unwrap()
    });
    static FUNC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:[\w:]+\s+)+([a-zA-Z_]\w*)\s*\(").unwrap()
    });

    const NOISE: &[&str] = &["main", "init", "test", "setup"];

    struct Sym { name: String, priority: u8 }
    let mut symbols: Vec<Sym> = Vec::new();

    // Header guard
    if let Some(cap) = HEADER_GUARD_RE.captures(content) {
        let guard = &cap[1];
        let stem = guard.strip_suffix("_HPP").or_else(|| guard.strip_suffix("_HXX"))
            .or_else(|| guard.strip_suffix("_H")).unwrap_or(guard);
        if !stem.is_empty() {
            return Some(stem.to_lowercase().replace('_', "-"));
        }
    }

    for cap in NAMESPACE_RE.captures_iter(content).take(2) {
        symbols.push(Sym { name: cap[1].to_string(), priority: 10 });
    }
    for cap in CLASS_RE.captures_iter(content).take(3) {
        symbols.push(Sym { name: cap[1].to_string(), priority: 9 });
    }
    for cap in FUNC_RE.captures_iter(content).take(3) {
        let name = cap[1].to_string();
        if !NOISE.contains(&name.as_str()) {
            symbols.push(Sym { name, priority: 7 });
        }
    }

    symbols.sort_by(|a, b| b.priority.cmp(&a.priority));

    let mut seen = HashSet::new();
    let mut tokens: Vec<String> = Vec::new();
    for sym in &symbols {
        if tokens.len() >= MAX_TOKENS { break; }
        if seen.insert(sym.name.clone()) {
            tokens.push(sym.name.clone());
        }
    }

    if tokens.is_empty() { None } else { Some(tokens.join("-")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_cpp(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn namespace_and_class() {
        let src = "namespace rendering {\nclass SceneGraph {\npublic:\n    void render();\n};\n}";
        let n = name(src).unwrap();
        assert!(n.contains("rendering"), "got: {n}");
    }

    #[test]
    fn template_class() {
        let src = "template <typename T>\nclass SmartPointer {\n    T* ptr;\n};";
        let n = name(src).unwrap();
        assert!(n.contains("smart-pointer"), "got: {n}");
    }

    #[test]
    fn header_guard() {
        let src = "#ifndef UTILS_HPP\n#define UTILS_HPP\nvoid helper();\n#endif";
        let n = name(src).unwrap();
        assert!(n.contains("utils"), "got: {n}");
    }

    #[test]
    fn cpp_class_via_tree_sitter() {
        let src = "#include <string>\n\nnamespace network {\nclass HttpClient {\npublic:\n    void get(const std::string& url);\nprivate:\n    std::string base_url;\n};\n}";
        let n = name(src).unwrap();
        assert!(
            n.contains("network") || n.contains("http-client"),
            "got: {n}"
        );
    }
}
