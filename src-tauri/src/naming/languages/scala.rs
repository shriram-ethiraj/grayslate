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
/// Priority order (file-local symbols outrank package context):
///   1. `object` / `case class` / `sealed trait` / `class` / `trait` — P9
///      (handles scoped visibility like `private[pkg] object Foo`)
///   2. `type` aliases — P8
///   3. `package object` — P8
///   4. Top-level `def` declarations — P7
///   5. Top-level `val` with PascalCase — P6
///   6. `package` declaration (last segment) — P5 (fallback context)
fn extract_scala(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static PACKAGE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^package\s+([\w.]+)").unwrap());
    // Handles: class Foo, private[pkg] object Foo, abstract sealed trait Foo
    static TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?m)^(?:(?:abstract|sealed|final|implicit|lazy|private(?:\[\w+\])?|protected(?:\[\w+\])?|override)\s+)*(?:case\s+)?(?:class|trait|object)\s+([A-Z][a-zA-Z0-9_]*)",
        )
        .unwrap()
    });
    // package object foo { ... }
    static PKG_OBJECT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^package\s+object\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap()
    });
    // type Foo = Bar
    static TYPE_ALIAS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^[ \t]*(?:(?:private|protected|override)\s+)*type\s+([A-Z][a-zA-Z0-9_]*)").unwrap()
    });
    static DEF_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:(?:private(?:\[\w+\])?|protected(?:\[\w+\])?|override|implicit|final|lazy)\s+)*def\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap()
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

    // Types: class/trait/object (P9)
    for cap in TYPE_RE.captures_iter(content).take(4) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 9 });
    }

    // package object (P8)
    if let Some(cap) = PKG_OBJECT_RE.captures(content) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 8 });
    }

    // Type alias (P8)
    for cap in TYPE_ALIAS_RE.captures_iter(content).take(3) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 8 });
    }

    // Top-level defs (P7)
    for cap in DEF_RE.captures_iter(content).take(4) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 7 });
    }

    // Package (last segment) — fallback context (P5)
    if let Some(cap) = PACKAGE_RE.captures(content) {
        if let Some(pkg) = cap[1].rsplit('.').next() {
            if !pkg.is_empty() {
                symbols.push(Symbol { name: pkg.to_string(), priority: 5 });
            }
        }
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

    // --- Priority rebalance: type outranks package ---
    #[test]
    fn case_class_leads_over_package() {
        let src = "package com.example.model\n\ncase class User(name: String, age: Int)";
        let n = name(src).unwrap();
        assert!(n.contains("user"), "case class wins over package: {n}");
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

    // --- New: scoped visibility ---
    #[test]
    fn scoped_visibility_object() {
        let src = "package org.scalatra.test\n\nprivate[scalatra] object EmbeddedJettyContainerCompat {\n  def configureServletContextHandler() = {}\n}";
        let n = name(src).unwrap();
        assert!(n.contains("embedded-jetty-container-compat"), "scoped visibility: {n}");
    }

    #[test]
    fn scoped_visibility_trait() {
        let src = "package org.scalatra\n\nprivate[scalatra] trait HasMultipartConfig";
        let n = name(src).unwrap();
        assert!(n.contains("has-multipart-config"), "scoped trait: {n}");
    }

    // --- New: package object ---
    #[test]
    fn package_object() {
        let src = "package org.scalatra\n\npackage object servlet {\n  type Config = ServletContext\n}";
        let n = name(src).unwrap();
        assert!(n.contains("servlet"), "package object: {n}");
    }

    // --- New: type alias ---
    #[test]
    fn type_alias() {
        let src = "package org.scalatra\n\ntype RouteTransformer = (Route => Route)";
        let n = name(src).unwrap();
        assert!(n.contains("route-transformer"), "type alias: {n}");
    }

    // --- Package-only fallback ---
    #[test]
    fn package_only_when_no_symbols() {
        let src = "package org.scalatra.util\n\nimport scala.collection._\n";
        let n = name(src).unwrap();
        assert!(n.contains("util"), "package fallback: {n}");
    }
}

