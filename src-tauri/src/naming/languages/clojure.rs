use super::NamingDefinition;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "clojure",
        extension: "clj",
        extract: extract_clojure,
    }
}

/// Clojure naming: (ns ...) namespace, (defn ...), (def ...), (defmacro ...).
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

    let mut tokens: Vec<String> = Vec::new();

    // Namespace (P10)
    if let Some(cap) = NS_RE.captures(content) {
        let ns = &cap[1];
        // Take last segment: my.app.core → core
        let short = ns.rsplit('.').next().unwrap_or(ns);
        tokens.push(short.to_string());
    }

    // defmacro (P9)
    for cap in DEFMACRO_RE.captures_iter(content).take(2) {
        if tokens.len() >= MAX_TOKENS { break; }
        tokens.push(cap[1].to_string());
    }

    // defn (P7)
    for cap in DEFN_RE.captures_iter(content).take(3) {
        if tokens.len() >= MAX_TOKENS { break; }
        let name = &cap[1];
        if name != "-main" && name != "main" {
            tokens.push(name.to_string());
        }
    }

    // def (P5) — only if we still need tokens
    if tokens.len() < MAX_TOKENS {
        for cap in DEF_RE.captures_iter(content).take(2) {
            if tokens.len() >= MAX_TOKENS { break; }
            tokens.push(cap[1].to_string());
        }
    }

    if tokens.is_empty() { None } else { Some(tokens.join("-")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_clojure(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn namespace_and_defn() {
        let src = "(ns my.app.auth\n  (:require [clojure.string :as str]))\n\n(defn authenticate [user pass]\n  (verify user pass))";
        let n = name(src).unwrap();
        assert!(n.contains("auth"), "got: {n}");
    }

    #[test]
    fn defmacro() {
        let src = "(ns utils)\n(defmacro with-timing [expr]\n  `(let [start# (System/nanoTime)]\n     ~expr))";
        let n = name(src).unwrap();
        assert!(n.contains("utils"), "got: {n}");
    }
}
