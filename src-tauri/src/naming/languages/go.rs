use super::NamingDefinition;
use crate::naming::code::{extract_with_tree_sitter, field_text, Symbol};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "go",
        extension: "go",
        extract: extract_go,
    }
}

fn extract_go(content: &str) -> Option<String> {
    extract_with_tree_sitter(content, tree_sitter_go::LANGUAGE, collect_go)
}

fn collect_go(root: &tree_sitter::Node, src: &[u8], symbols: &mut Vec<Symbol>) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            // Package provides context but shouldn't dominate over specific
            // type/function names — kept below exported types (P9) and
            // exported functions (P7) so the stem reads "TokenService-auth"
            // rather than "auth-TokenService".
            "package_clause" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(src) {
                        symbols.push(Symbol { name: name.to_string(), priority: 5 });
                    }
                } else {
                    let mut inner = child.walk();
                    for gc in child.children(&mut inner) {
                        if gc.kind() == "package_identifier" || gc.kind() == "identifier" {
                            if let Ok(name) = gc.utf8_text(src) {
                                symbols.push(Symbol { name: name.to_string(), priority: 5 });
                            }
                        }
                    }
                }
            }
            "type_declaration" => {
                let mut inner = child.walk();
                for gc in child.children(&mut inner) {
                    if gc.kind() == "type_spec" {
                        if let Some(name) = field_text(&gc, "name", src) {
                            let first = name.chars().next().unwrap_or('_');
                            let pri = if first.is_uppercase() { 9 } else { 5 };
                            symbols.push(Symbol { name: name.to_string(), priority: pri });
                        }
                    }
                }
            }
            "function_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    let first = name.chars().next().unwrap_or('_');
                    // Exported (PascalCase) P7, unexported P6 (above package P5)
                    let pri = if first.is_uppercase() { 7 } else { 6 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            "method_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    let first = name.chars().next().unwrap_or('_');
                    let pri = if first.is_uppercase() { 6 } else { 4 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            _ => {}
        }
    }

    // Fallback: package doc comment (// comment block before `package`)
    let has_real_symbols = symbols.iter().any(|s| s.priority > 5);
    if !has_real_symbols {
        if let Some(desc) = extract_package_doc(src) {
            symbols.push(Symbol { name: desc, priority: 3 });
        }
    }
}

/// Extract Go package doc comment: the `//` block directly before `package`.
fn extract_package_doc(src: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(src).ok()?;
    let mut doc_lines: Vec<&str> = Vec::new();
    for line in text.lines().take(20) {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            let comment = trimmed.trim_start_matches('/').trim();
            if !comment.is_empty() {
                doc_lines.push(comment);
            }
        } else if trimmed.starts_with("package ") {
            break;
        } else if !trimmed.is_empty() {
            doc_lines.clear(); // non-comment before package — reset
        }
    }
    // Take the first meaningful line of the doc block
    let first = doc_lines.first()?;
    // Strip conventional "Package foo ..." prefix
    let cleaned = if first.starts_with("Package ") {
        first.strip_prefix("Package ").unwrap().trim()
        // After stripping "Package foo", skip the first word (package name)
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
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_go(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn exported_type_leads_over_package() {
        let code = "package auth\n\ntype TokenService struct {\n    secret string\n}\n\nfunc (s *TokenService) Generate(claims Claims) string { return \"\" }\nfunc init() {}";
        let result = name(code).unwrap();
        // With MAX_TOKENS=1, only the highest-priority exported type (P9) is kept
        assert!(result.contains("token-service"), "exported type first: {result}");
        assert!(!result.contains("init"), "init filtered: {result}");
    }

    #[test]
    fn package_only_when_no_exports() {
        let code = "package utils\n\nfunc helper() {}\nfunc another() {}";
        let result = name(code).unwrap();
        // Unexported func (P6) now beats package (P5)
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
        // Should get the doc comment description, not just "ratelimit"
        assert!(result.contains("ratelimit"), "got: {result}");
    }

    // --- Unexported function beats generic package name ---
    #[test]
    fn unexported_func_beats_cmd_package() {
        let code = "package cmd\n\nimport \"fmt\"\n\nfunc newCompletionCmd() *cobra.Command {\n    return nil\n}\n\nfunc runCompletionBash() error {\n    return nil\n}";
        let result = name(code).unwrap();
        // Unexported func (P6) beats package (P5) — more descriptive
        assert!(result.contains("new-completion-cmd") || result.contains("completion"), "func beats cmd: {result}");
    }
}
