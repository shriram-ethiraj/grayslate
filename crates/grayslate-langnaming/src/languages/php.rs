use std::collections::HashSet;

use super::{NamingDefinition, Extractor};
use crate::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "php",
        extension: "php",
        extract: Extractor::Custom(extract_php),
    }
}

/// PHP naming extraction with namespace awareness.
///
/// Priority order (file-local types outrank namespace context):
///   1. `class` / `interface` / `trait` / `enum` — P9
///   2. `function` declarations — P7
///   3. `namespace` (last segment) — P5 (fallback context)
fn extract_php(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static NAMESPACE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^namespace\s+([A-Za-z_][\w\\]+)").unwrap());
    static TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?m)^(?:abstract\s+|final\s+|readonly\s+)*(?:class|interface|trait|enum)\s+([A-Z]\w+)",
        )
        .unwrap()
    });
    static FUNC_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^function\s+([a-zA-Z_]\w+)\s*\(").unwrap());

    const NOISE: &[&str] = &[
        "Test", "Tests", "TestCase", "ServiceProvider", "Facade",
        "main", "index", "register", "boot", "handle",
    ];

    struct Symbol { name: String, priority: u8 }
    let mut symbols: Vec<Symbol> = Vec::new();

    // Types (P9) — highest priority
    for cap in TYPE_RE.captures_iter(content).take(4) {
        let name = cap[1].to_string();
        if !NOISE.contains(&name.as_str()) {
            symbols.push(Symbol { name, priority: 9 });
        }
    }

    // Functions (P7)
    for cap in FUNC_RE.captures_iter(content).take(4) {
        let name = cap[1].to_string();
        if !NOISE.contains(&name.as_str()) {
            symbols.push(Symbol { name, priority: 7 });
        }
    }

    // Namespace → last segment (P5, fallback)
    if let Some(cap) = NAMESPACE_RE.captures(content) {
        if let Some(ns) = cap[1].rsplit('\\').next() {
            if !ns.is_empty() && !NOISE.contains(&ns) {
                symbols.push(Symbol { name: ns.to_string(), priority: 5 });
            }
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
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_php(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn class_leads_over_namespace() {
        let src = "<?php\nnamespace App\\Models;\n\nclass User extends Model {\n  public function name() {}\n}";
        let n = name(src).unwrap();
        assert!(n.contains("user"), "class wins over namespace: {n}");
    }

    #[test]
    fn interface() {
        let src = "<?php\ninterface Cacheable {\n  public function getCacheKey(): string;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("cacheable"), "got: {n}");
    }

    #[test]
    fn trait_extraction() {
        let src = "<?php\nnamespace App\\Traits;\n\ntrait HasPermissions {\n  public function can() {}\n}";
        let n = name(src).unwrap();
        assert!(n.contains("has-permissions"), "trait wins over ns: {n}");
    }

    // --- Audit regression: class beats Illuminate namespace ---
    #[test]
    fn class_beats_illuminate_namespace() {
        let src = "<?php\nnamespace Illuminate\\Auth;\n\nclass AuthManager {\n    public function guard() {}\n}";
        let n = name(src).unwrap();
        assert!(n.contains("auth-manager"), "class beats Illuminate: {n}");
    }

    #[test]
    fn enum_extraction() {
        let src = "<?php\nnamespace App\\Enums;\n\nenum Status: string {\n    case Active = 'active';\n}";
        let n = name(src).unwrap();
        assert!(n.contains("status"), "enum wins over ns: {n}");
    }

    // --- Namespace-only fallback ---
    #[test]
    fn namespace_only_when_no_types() {
        let src = "<?php\nnamespace Illuminate\\Support;\n\n// helpers";
        let n = name(src).unwrap();
        assert!(n.contains("support"), "namespace fallback: {n}");
    }

    #[test]
    fn function_wins_over_namespace() {
        let src = "<?php\nnamespace App\\Helpers;\n\nfunction format_currency($amount) {\n    return '$' . number_format($amount, 2);\n}";
        let n = name(src).unwrap();
        assert!(n.contains("format-currency"), "function wins: {n}");
    }
}
