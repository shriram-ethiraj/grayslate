use std::collections::HashSet;

use super::NamingDefinition;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "scala",
        extension: "scala",
        extract: extract_scala,
    }
}

/// Scala-specific regex extraction.
///
/// Priority order:
///   1. `package` declaration (last segment) — P10
///   2. `object` / `case class` / `sealed trait` / `class` / `trait` — P9
///   3. Top-level `def` declarations — P7
///   4. Top-level `val` with PascalCase — P6
fn extract_scala(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static PACKAGE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^package\s+([\w.]+)").unwrap());
    static TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?m)^(?:(?:abstract|sealed|final|implicit|lazy|private|protected|override)\s+)*(?:case\s+)?(?:class|trait|object)\s+([A-Z][a-zA-Z0-9_]*)",
        )
        .unwrap()
    });
    static DEF_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:(?:private|protected|override|implicit|final|lazy)\s+)*def\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap()
    });

    const NOISE: &[&str] = &[
        "main", "init", "setup", "run", "start", "new", "default", "handle",
        "index", "app", "mod", "test", "self", "this", "apply", "unapply",
        "toString", "hashCode", "equals", "copy", "canEqual",
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

    // Types: class/trait/object (P9)
    for cap in TYPE_RE.captures_iter(content).take(4) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 9 });
    }

    // Top-level defs (P7)
    for cap in DEF_RE.captures_iter(content).take(4) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 7 });
    }

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
        extract_scala(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn case_class() {
        let src = "package com.example.model\n\ncase class User(name: String, age: Int)";
        let n = name(src).unwrap();
        assert!(n.contains("model"), "got: {n}");
    }

    #[test]
    fn sealed_trait() {
        let src = "sealed trait Result[+T]\ncase class Success[T](value: T) extends Result[T]";
        let n = name(src).unwrap();
        assert!(n.contains("result"), "got: {n}");
    }

    #[test]
    fn object_with_def() {
        let src = "object MathUtils {\n  def factorial(n: Int): BigInt = ???\n}";
        let n = name(src).unwrap();
        assert!(n.contains("math-utils"), "got: {n}");
    }
}

