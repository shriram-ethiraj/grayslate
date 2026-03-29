use super::{NamingDefinition, Extractor};
use crate::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "clojure",
        extension: "clj",
        extract: Extractor::Custom(extract_clojure),
    }
}

/// Clojure naming: (ns ...) namespace, (defn ...), (def ...), (defmacro ...),
/// (defproject ...), (defrecord ...), (deftype ...), (deftest ...).
///
/// Priority order:
///   1. `defproject` — P10 (Leiningen project.clj, most specific)
///   2. `defrecord` / `deftype` — P9 (concrete types)
///   3. `defmacro` — P9
///   4. `deftest` — P8 (test name is meaningful)
///   5. `defn` — P7
///   6. `ns` namespace (last segment) — P5 (fallback context)
///   7. `def` — P4
fn extract_clojure(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static NS_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)\(ns\s+([\w.\-]+)").unwrap());
    static DEFN_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)\(defn-?\s+([\w\-?!]+)").unwrap());
    static DEF_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)\(def\s+([\w\-?!]+)").unwrap());
    static DEFMACRO_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)\(defmacro\s+([\w\-?!]+)").unwrap());
    // Leiningen project.clj: (defproject group/artifact "version" ...)
    static DEFPROJECT_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)\(defproject\s+([\w.\-/]+)").unwrap());
    // defrecord / deftype — concrete type definitions
    static DEFRECORD_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)\(defrecord\s+([A-Z][\w\-]*)").unwrap());
    static DEFTYPE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)\(deftype\s+([A-Z][\w\-]*)").unwrap());
    // deftest — test names
    static DEFTEST_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)\(deftest\s+([\w\-?!]+)").unwrap());
    // defprotocol — protocol definitions (like interfaces)
    static DEFPROTOCOL_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)\(defprotocol\s+([A-Z][\w\-]*)").unwrap());
    // defmulti — multimethod dispatch definitions
    static DEFMULTI_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)\(defmulti\s+([\w\-?!]+)").unwrap());

    struct Symbol { name: String, priority: u8 }
    let mut symbols: Vec<Symbol> = Vec::new();

    // defproject (P10) — early return for Leiningen project files
    if let Some(cap) = DEFPROJECT_RE.captures(content) {
        let project = &cap[1];
        // group/artifact → take artifact; plain name → use as-is
        let short = project.rsplit('/').next().unwrap_or(project);
        return Some(short.to_string());
    }

    // defrecord / deftype (P9) — concrete types
    for cap in DEFRECORD_RE.captures_iter(content).take(3) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 9 });
    }
    for cap in DEFTYPE_RE.captures_iter(content).take(3) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 9 });
    }

    // defprotocol (P9) — protocol definitions
    for cap in DEFPROTOCOL_RE.captures_iter(content).take(3) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 9 });
    }

    // defmacro (P9)
    for cap in DEFMACRO_RE.captures_iter(content).take(2) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 9 });
    }

    // deftest (P8)
    for cap in DEFTEST_RE.captures_iter(content).take(3) {
        let name = cap[1].to_string();
        if name != "test" {
            symbols.push(Symbol { name, priority: 8 });
        }
    }

    // defmulti (P8) — multimethod definitions
    for cap in DEFMULTI_RE.captures_iter(content).take(3) {
        let name = cap[1].to_string();
        if name != "main" && name != "-main" {
            symbols.push(Symbol { name, priority: 8 });
        }
    }

    // defn (P7)
    for cap in DEFN_RE.captures_iter(content).take(3) {
        let name = &cap[1];
        if name != "-main" && name != "main" {
            symbols.push(Symbol { name: name.to_string(), priority: 7 });
        }
    }

    // Namespace (last segment) — fallback context (P5)
    if let Some(cap) = NS_RE.captures(content) {
        let ns = &cap[1];
        let short = ns.rsplit('.').next().unwrap_or(ns);
        if !short.is_empty() {
            symbols.push(Symbol { name: short.to_string(), priority: 5 });
        }
    }

    // def (P4) — only if we still need tokens
    if symbols.is_empty() || !symbols.iter().any(|s| s.priority > 5) {
        for cap in DEF_RE.captures_iter(content).take(2) {
            if symbols.len() + 1 > MAX_TOKENS * 3 { break; }
            symbols.push(Symbol { name: cap[1].to_string(), priority: 4 });
        }
    }

    symbols.sort_by(|a, b| b.priority.cmp(&a.priority));

    let mut seen = std::collections::HashSet::new();
    let mut tokens: Vec<String> = Vec::new();
    for sym in &symbols {
        if tokens.len() >= MAX_TOKENS { break; }
        let lower = sym.name.to_lowercase();
        if lower != "main" && lower != "-main" && seen.insert(sym.name.clone()) {
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
        extract_clojure(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn namespace_and_defn() {
        let src = "(ns my.app.auth\n  (:require [clojure.string :as str]))\n\n(defn authenticate [user pass]\n  (verify user pass))";
        let n = name(src).unwrap();
        assert!(n.contains("authenticate"), "defn wins over ns: {n}");
    }

    #[test]
    fn defmacro() {
        let src = "(ns utils)\n(defmacro with-timing [expr]\n  `(let [start# (System/nanoTime)]\n     ~expr))";
        let n = name(src).unwrap();
        assert!(n.contains("with-timing"), "defmacro wins over ns: {n}");
    }

    // --- New: defproject ---
    #[test]
    fn defproject_leiningen() {
        let src = r#"(defproject compojure "1.7.1"
  :description "A concise routing library for Ring"
  :url "https://github.com/weavejester/compojure"
  :dependencies [[org.clojure/clojure "1.9.0"]])"#;
        let n = name(src).unwrap();
        assert!(n.contains("compojure"), "defproject extracted: {n}");
    }

    #[test]
    fn defproject_with_group() {
        let src = r#"(defproject ring/ring-core "1.9.0"
  :description "Core Ring library")"#;
        let n = name(src).unwrap();
        assert!(n.contains("ring-core"), "group/artifact → artifact: {n}");
    }

    // --- New: defrecord ---
    #[test]
    fn defrecord() {
        let src = "(ns my.app.model)\n(defrecord User [name email])";
        let n = name(src).unwrap();
        assert!(n.contains("user"), "defrecord extracted: {n}");
    }

    // --- New: deftype ---
    #[test]
    fn deftype() {
        let src = "(ns my.app.types)\n(deftype CustomQueue [head tail])";
        let n = name(src).unwrap();
        assert!(n.contains("custom-queue"), "deftype extracted: {n}");
    }

    // --- New: deftest ---
    #[test]
    fn deftest() {
        let src = "(ns my.app.auth-test\n  (:require [clojure.test :refer :all]))\n\n(deftest authentication-flow\n  (is (= true (auth/check \"user\" \"pass\"))))";
        let n = name(src).unwrap();
        assert!(n.contains("authentication-flow"), "deftest extracted: {n}");
    }

    // --- Namespace-only fallback ---
    #[test]
    fn namespace_only_when_no_defs() {
        let src = "(ns my.app.core\n  (:require [clojure.string :as str]))";
        let n = name(src).unwrap();
        assert!(n.contains("core"), "ns fallback: {n}");
    }

    #[test]
    fn defprotocol() {
        let src = "(ns my.protocols)\n(defprotocol Serializable\n  (serialize [this])\n  (deserialize [this data]))";
        let n = name(src).unwrap();
        assert!(n.contains("serializable"), "defprotocol: {n}");
    }

    #[test]
    fn defmulti() {
        let src = "(ns my.dispatch)\n(defmulti process-event :type)\n(defmethod process-event :click [e] (handle-click e))";
        let n = name(src).unwrap();
        assert!(n.contains("process-event"), "defmulti: {n}");
    }
}
