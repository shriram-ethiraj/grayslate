use std::collections::HashSet;

use super::model::MAX_TOKENS;
use tree_sitter::Parser;
use tree_sitter_language::LanguageFn;

// ---------------------------------------------------------------------------
// Shared code-extraction utilities.
//
// Language-specific logic lives in `languages/<lang>.rs`. This module provides
// reusable building blocks: Symbol, noise filtering, tree-sitter helpers, and
// the `symbols_to_stem` finaliser that every language extractor can call.
// ---------------------------------------------------------------------------

/// Noise symbol names that are too generic to be useful in a filename.
const NOISE_NAMES: &[&str] = &[
    "main", "init", "setup", "run", "start", "new", "default", "handle",
    "index", "app", "mod", "test", "tests", "self", "this", "cls",
    "definition",
];

pub(crate) fn is_noise_name(name: &str) -> bool {
    NOISE_NAMES.contains(&name)
}

/// A collected symbol with its naming priority.
pub(crate) struct Symbol {
    pub name: String,
    pub priority: u8,
}

/// Sort symbols by priority, dedup, filter noise, and join into a stem.
pub(crate) fn symbols_to_stem(symbols: &mut Vec<Symbol>) -> Option<String> {
    symbols.sort_by(|a, b| b.priority.cmp(&a.priority));

    let mut seen = HashSet::new();
    let mut tokens: Vec<String> = Vec::new();
    for sym in symbols.iter() {
        if tokens.len() >= MAX_TOKENS {
            break;
        }
        if !is_noise_name(&sym.name) && seen.insert(sym.name.clone()) {
            tokens.push(sym.name.clone());
        }
    }

    if tokens.is_empty() { None } else { Some(tokens.join("-")) }
}

// ---------------------------------------------------------------------------
// tree-sitter helpers
// ---------------------------------------------------------------------------

/// Parse content with a tree-sitter grammar, collect symbols via a callback,
/// and return the finalised stem.
pub(crate) fn extract_with_tree_sitter(
    content: &str,
    language_fn: LanguageFn,
    collector: fn(&tree_sitter::Node, &[u8], &mut Vec<Symbol>),
) -> Option<String> {
    let mut parser = Parser::new();
    parser.set_language(&language_fn.into()).ok()?;
    let tree = parser.parse(content, None)?;
    let root = tree.root_node();
    let src = content.as_bytes();

    let mut symbols: Vec<Symbol> = Vec::new();
    collector(&root, src, &mut symbols);
    symbols_to_stem(&mut symbols)
}

/// Extract text of a named field from a tree-sitter node.
pub(crate) fn field_text<'a>(
    node: &tree_sitter::Node,
    field: &str,
    src: &'a [u8],
) -> Option<&'a str> {
    node.child_by_field_name(field)?
        .utf8_text(src)
        .ok()
}

/// Check whether a Rust node has a `pub` visibility modifier as a direct child.
pub(crate) fn has_pub_child(node: &tree_sitter::Node, src: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            if let Ok(text) = child.utf8_text(src) {
                if text.starts_with("pub") {
                    return true;
                }
            }
        }
    }
    false
}

/// Recursively descend a C/C++ declarator to find the identifier name.
/// Handles nested declarators like `(*func_ptr)(...)` and `ClassName::method(...)`.
pub(crate) fn extract_identifier_from_declarator(
    node: &tree_sitter::Node,
    src: &[u8],
) -> Option<String> {
    match node.kind() {
        "identifier" => node.utf8_text(src).ok().map(|s| s.to_string()),
        "field_identifier" => node.utf8_text(src).ok().map(|s| s.to_string()),
        "qualified_identifier" | "scoped_identifier" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                return name_node.utf8_text(src).ok().map(|s| s.to_string());
            }
            None
        }
        _ => {
            if let Some(inner) = node.child_by_field_name("declarator") {
                return extract_identifier_from_declarator(&inner, src);
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" || child.kind() == "field_identifier" {
                    return child.utf8_text(src).ok().map(|s| s.to_string());
                }
            }
            None
        }
    }
}

/// Heuristic to pick C++ grammar when content looks like C++.
pub(crate) fn pick_c_or_cpp_grammar(content: &str) -> LanguageFn {
    let sample = if content.len() > 2000 { &content[..2000] } else { content };
    if sample.contains("class ")
        || sample.contains("namespace ")
        || sample.contains("template<")
        || sample.contains("template <")
        || sample.contains("::")
        || sample.contains("std::")
        || sample.contains("#include <iostream>")
        || sample.contains("#include <vector>")
        || sample.contains("#include <string>")
    {
        tree_sitter_cpp::LANGUAGE
    } else {
        tree_sitter_c::LANGUAGE
    }
}

/// Regex-based fallback: run a list of patterns, collect captures into symbols.
pub(crate) fn extract_with_regex(
    content: &str,
    patterns: &[(&str, u8)],  // (pattern, priority)
) -> Option<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut tokens: Vec<String> = Vec::new();

    for &(pattern, _priority) in patterns {
        if tokens.len() >= MAX_TOKENS {
            break;
        }
        if let Ok(re) = regex::Regex::new(pattern) {
            for cap in re.captures_iter(content).take(3) {
                if tokens.len() >= MAX_TOKENS {
                    break;
                }
                if let Some(m) = cap.get(1) {
                    let name = m.as_str().to_string();
                    if !name.is_empty() && !is_noise_name(&name) && seen.insert(name.clone()) {
                        tokens.push(name);
                    }
                }
            }
        }
    }

    if tokens.is_empty() { None } else { Some(tokens.join("-")) }
}

// ---------------------------------------------------------------------------
// Tests — utility functions only. Language-specific tests live in
// `languages/<lang>.rs`.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_filtering() {
        assert!(is_noise_name("main"));
        assert!(is_noise_name("init"));
        assert!(is_noise_name("setup"));
        assert!(is_noise_name("test"));
        assert!(is_noise_name("tests"));
        assert!(is_noise_name("definition"));
        assert!(!is_noise_name("UserAuth"));
        assert!(!is_noise_name("parse_csv"));
    }

    #[test]
    fn symbols_to_stem_sorts_and_dedupes() {
        let mut syms = vec![
            Symbol { name: "low".into(), priority: 3 },
            Symbol { name: "high".into(), priority: 9 },
            Symbol { name: "high".into(), priority: 9 }, // duplicate
            Symbol { name: "mid".into(), priority: 5 },
        ];
        let result = symbols_to_stem(&mut syms).unwrap();
        assert!(result.starts_with("high"), "highest priority first: {result}");
        // "high" appears only once
        assert_eq!(result.matches("high").count(), 1);
    }

    #[test]
    fn symbols_to_stem_filters_noise() {
        let mut syms = vec![
            Symbol { name: "main".into(), priority: 10 },
            Symbol { name: "Config".into(), priority: 5 },
        ];
        let result = symbols_to_stem(&mut syms).unwrap();
        assert!(!result.contains("main"), "noise filtered: {result}");
        assert!(result.contains("Config"), "non-noise kept: {result}");
    }

    #[test]
    fn symbols_to_stem_empty_returns_none() {
        let mut syms: Vec<Symbol> = Vec::new();
        assert!(symbols_to_stem(&mut syms).is_none());
    }

    #[test]
    fn regex_helper_basic() {
        let content = "export class UserAuth {\n}\nexport function createToken() {}";
        let patterns: &[(&str, u8)] = &[
            (r"(?m)^export\s+class\s+([A-Za-z_]\w*)", 9),
            (r"(?m)^export\s+function\s+([a-zA-Z_]\w+)", 7),
        ];
        let result = extract_with_regex(content, patterns).unwrap();
        assert!(result.contains("UserAuth"), "got: {result}");
    }
}
