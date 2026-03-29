use super::{NamingDefinition, Extractor};
use crate::code::{is_noise_name, symbols_to_stem, Symbol};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "go",
        extension: "go",
        extract: Extractor::Custom(extract_go),
    }
}

fn extract_go(content: &str) -> Option<String> {
    extract_go_regex(content)
}

/// Regex-based Go naming: package, type, func, method, package doc.
fn extract_go_regex(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static PACKAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^package\s+([a-zA-Z_]\w*)").unwrap()
    });
    // type TokenService struct { ... } or type Reader interface { ... }
    static TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^type\s+([A-Za-z_]\w*)\s+(?:struct|interface)\b").unwrap()
    });
    // type Alias = OtherType
    static TYPE_ALIAS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^type\s+([A-Z][A-Za-z0-9_]*)\s+\w").unwrap()
    });
    // var ErrNotFound = errors.New(...)
    static VAR_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^var\s+([A-Z][A-Za-z0-9_]*)\s").unwrap()
    });
    // func HandleRequest(...) or func main()
    static FUNC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^func\s+([A-Za-z_]\w*)\s*\(").unwrap()
    });
    // func (s *TokenService) Generate(...) — method receiver
    static METHOD_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^func\s+\([^)]+\)\s+([A-Za-z_]\w*)\s*\(").unwrap()
    });

    let mut symbols: Vec<Symbol> = Vec::new();

    // Package clause
    if let Some(cap) = PACKAGE_RE.captures(content) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 5 });
    }

    // Type declarations (struct/interface — highest priority)
    for cap in TYPE_RE.captures_iter(content).take(3) {
        let name = &cap[1];
        let first = name.chars().next().unwrap_or('_');
        let pri = if first.is_uppercase() { 9 } else { 5 };
        symbols.push(Symbol { name: name.to_string(), priority: pri });
    }

    // Type aliases and custom types (lower than struct/interface)
    for cap in TYPE_ALIAS_RE.captures_iter(content).take(2) {
        let name = &cap[1];
        if !symbols.iter().any(|s| s.name == *name) {
            symbols.push(Symbol { name: name.to_string(), priority: 7 });
        }
    }

    // Exported var declarations (e.g., var ErrNotFound)
    for cap in VAR_RE.captures_iter(content).take(2) {
        let name = &cap[1];
        if !is_noise_name(name) {
            symbols.push(Symbol { name: name.to_string(), priority: 5 });
        }
    }

    // Function declarations
    for cap in FUNC_RE.captures_iter(content).take(3) {
        let name = &cap[1];
        let first = name.chars().next().unwrap_or('_');
        let pri = if first.is_uppercase() { 7 } else { 6 };
        if !is_noise_name(name) {
            symbols.push(Symbol { name: name.to_string(), priority: pri });
        }
    }

    // Method declarations
    for cap in METHOD_RE.captures_iter(content).take(3) {
        let name = &cap[1];
        let first = name.chars().next().unwrap_or('_');
        let pri = if first.is_uppercase() { 6 } else { 4 };
        symbols.push(Symbol { name: name.to_string(), priority: pri });
    }

    if let Some(stem) = symbols_to_stem(&mut symbols) {
        return Some(stem);
    }

    // Fallback: package doc comment
    if let Some(desc) = extract_package_doc_regex(content) {
        return Some(desc);
    }

    None
}

/// Extract Go package doc comment (regex version): the `//` block before `package`.
fn extract_package_doc_regex(content: &str) -> Option<String> {
    let mut doc_lines: Vec<&str> = Vec::new();
    for line in content.lines().take(20) {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            let comment = trimmed.trim_start_matches('/').trim();
            if !comment.is_empty() {
                doc_lines.push(comment);
            }
        } else if trimmed.starts_with("package ") {
            break;
        } else if !trimmed.is_empty() {
            doc_lines.clear();
        }
    }
    let first = doc_lines.first()?;
    let cleaned = if first.starts_with("Package ") {
        first.strip_prefix("Package ").unwrap().trim()
    } else {
        first
    };
    if cleaned.len() >= 5 && cleaned.len() <= 80 {
        Some(cleaned.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_go(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn exported_type_leads_over_package() {
        let code = "package auth\n\ntype TokenService struct {\n    secret string\n}\n\nfunc (s *TokenService) Generate(claims Claims) string { return \"\" }\nfunc init() {}";
        let result = name(code).unwrap();
        assert!(result.contains("token-service"), "exported type first: {result}");
        assert!(!result.contains("init"), "init filtered: {result}");
    }

    #[test]
    fn package_only_when_no_exports() {
        let code = "package utils\n\nfunc helper() {}\nfunc another() {}";
        let result = name(code).unwrap();
        assert!(result.contains("helper"), "unexported func wins: {result}");
    }

    #[test]
    fn exported_func_over_package() {
        let code = "package http\n\nfunc HandleRequest(w Writer, r *Request) {}\nfunc ServeHTTP() {}";
        let result = name(code).unwrap();
        assert!(result.starts_with("handle-request"), "exported func leads: {result}");
    }

    #[test]
    fn highest_priority_type() {
        let code = "package models\n\ntype User struct { Name string }\ntype UserRepository interface { FindByID(id int) *User }";
        let result = name(code).unwrap();
        assert!(result.contains("user"), "got: {result}");
    }

    #[test]
    fn package_doc_comment_fallback() {
        let code = "// Package ratelimit implements a token bucket rate limiter.\npackage ratelimit\n\nimport \"sync\"\n";
        let result = name(code).unwrap();
        assert!(result.contains("ratelimit"), "got: {result}");
    }

    #[test]
    fn unexported_func_beats_cmd_package() {
        let code = "package cmd\n\nimport \"fmt\"\n\nfunc newCompletionCmd() *cobra.Command {\n    return nil\n}\n\nfunc runCompletionBash() error {\n    return nil\n}";
        let result = name(code).unwrap();
        assert!(result.contains("new-completion-cmd") || result.contains("completion"), "func beats cmd: {result}");
    }

    #[test]
    fn interface_type() {
        let code = "package io\n\ntype Reader interface {\n    Read(p []byte) (n int, err error)\n}\n\ntype Writer interface {\n    Write(p []byte) (n int, err error)\n}";
        let result = name(code).unwrap();
        assert!(result.contains("reader"), "interface type: {result}");
    }

    #[test]
    fn var_declaration() {
        let code = "package main\n\nimport \"errors\"\n\nvar ErrNotFound = errors.New(\"not found\")\n";
        let result = name(code).unwrap();
        assert!(result.contains("err-not-found"), "var decl: {result}");
    }
}
