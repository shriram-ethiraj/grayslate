use std::collections::HashSet;

use super::NamingDefinition;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "kotlin",
        extension: "kt",
        extract: extract_kotlin,
    }
}

/// Kotlin-specific regex extraction.
///
/// Priority order:
///   1. `package` declaration (last segment) — P10
///   2. `object` / `data class` / `sealed class` / `class` / `interface` / `enum class` — P9
///   3. Top-level `fun` declarations — P7
///   4. `@Composable fun` — P8 (UI component, higher than plain fun)
///   5. Top-level `val`/`var` with PascalCase — P6
fn extract_kotlin(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static PACKAGE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^package\s+([\w.]+)").unwrap());
    static CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?m)^(?:(?:public|private|internal|protected|abstract|open|sealed|data|inner|value)\s+)*(?:class|interface|object|enum\s+class)\s+([A-Z][a-zA-Z0-9_]*)",
        )
        .unwrap()
    });
    static COMPOSABLE_FUN_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)@Composable\s+(?:(?:public|private|internal)\s+)?fun\s+([A-Z][a-zA-Z0-9_]*)").unwrap()
    });
    static FUN_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:(?:public|private|internal|protected|override|suspend|inline|operator)\s+)*fun\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap()
    });

    const NOISE: &[&str] = &[
        "main", "init", "setup", "run", "start", "new", "default", "handle",
        "index", "app", "mod", "test", "self", "this", "invoke", "apply",
        "onCreate", "onStart", "onResume", "onPause", "onStop", "onDestroy",
        "toString", "hashCode", "equals", "copy", "component1",
    ];

    struct Symbol {
        name: String,
        priority: u8,
    }

    let mut symbols: Vec<Symbol> = Vec::new();

    // Package (last segment)
    if let Some(cap) = PACKAGE_RE.captures(content) {
        if let Some(pkg) = cap[1].rsplit('.').next() {
            if !pkg.is_empty() {
                symbols.push(Symbol { name: pkg.to_string(), priority: 10 });
            }
        }
    }

    // Composable functions (P8)
    for cap in COMPOSABLE_FUN_RE.captures_iter(content).take(4) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 8 });
    }

    // Classes/objects/interfaces (P9)
    for cap in CLASS_RE.captures_iter(content).take(4) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 9 });
    }

    // Top-level functions (P7)
    for cap in FUN_RE.captures_iter(content).take(4) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 7 });
    }

    // Sort by priority descending, deduplicate, filter noise.
    symbols.sort_by(|a, b| b.priority.cmp(&a.priority));

    let mut seen = HashSet::new();
    let mut tokens: Vec<String> = Vec::new();
    for sym in &symbols {
        if tokens.len() >= MAX_TOKENS {
            break;
        }
        if !NOISE.contains(&sym.name.as_str()) && seen.insert(sym.name.clone()) {
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
        extract_kotlin(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn data_class() {
        let src = "package com.example.model\n\ndata class User(val name: String, val age: Int)";
        let n = name(src).unwrap();
        assert!(n.contains("model"), "got: {n}");
    }

    #[test]
    fn object_declaration() {
        let src = "object DatabaseHelper {\n    fun getConnection(): Connection { }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("database-helper"), "got: {n}");
    }

    #[test]
    fn sealed_class() {
        let src = "sealed class Result<out T> {\n    data class Success<T>(val data: T) : Result<T>()\n}";
        let n = name(src).unwrap();
        assert!(n.contains("result"), "got: {n}");
    }

    #[test]
    fn composable_function() {
        let src = "package com.example.ui\n\n@Composable\nfun UserProfile(user: User) { }";
        let n = name(src).unwrap();
        assert!(n.contains("ui"), "got: {n}");
    }

    #[test]
    fn top_level_fun() {
        let src = "fun calculateTotal(items: List<Item>): Double { return 0.0 }";
        let n = name(src).unwrap();
        assert!(n.contains("calculate-total"), "got: {n}");
    }
}

