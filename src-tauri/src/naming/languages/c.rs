use super::NamingDefinition;
use crate::naming::code::{
    extract_identifier_from_declarator, extract_with_tree_sitter, field_text,
    pick_c_or_cpp_grammar, Symbol,
};
use std::collections::HashSet;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "c",
        extension: "c",
        extract: extract_c,
    }
}

/// C naming: tree-sitter first, then regex fallback for headers/macros.
fn extract_c(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    // Try tree-sitter first
    if let Some(result) = extract_with_tree_sitter(
        content,
        pick_c_or_cpp_grammar(content),
        collect_c_cpp,
    ) {
        return Some(result);
    }

    // Regex fallback for patterns tree-sitter misses (headers, macros)
    static HEADER_GUARD_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^#ifndef\s+([A-Z][A-Z0-9_]+_H(?:PP|XX)?)").unwrap()
    });
    static DEFINE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^#define\s+([A-Z][A-Z0-9_]{2,})(?:\s|$|\()").unwrap()
    });
    static TYPEDEF_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)typedef\s+(?:struct|union|enum)?\s*\{[^}]*\}\s*([A-Za-z_]\w+)\s*;")
            .unwrap()
    });
    static TYPEDEF_SIMPLE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^typedef\s+\w+\s+([A-Za-z_]\w+)\s*;").unwrap()
    });
    static ENUM_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^enum\s+([A-Za-z_]\w*)").unwrap());
    static STRUCT_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^struct\s+([A-Za-z_]\w*)").unwrap());
    static FUNC_DECL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:static\s+)?(?:inline\s+)?(?:const\s+)?\w[\w\s*]+\s+\*?([a-zA-Z_]\w*)\s*\(")
            .unwrap()
    });

    const NOISE: &[&str] = &[
        "main", "init", "test", "setup", "TRUE", "FALSE", "NULL", "EOF",
        "MAX", "MIN", "SIZE", "LEN",
    ];

    struct Sym { name: String, priority: u8 }
    let mut symbols: Vec<Sym> = Vec::new();

    // Header guard → derive a name from it
    if let Some(cap) = HEADER_GUARD_RE.captures(content) {
        let guard = &cap[1];
        let stem = guard
            .strip_suffix("_HPP").or_else(|| guard.strip_suffix("_HXX"))
            .or_else(|| guard.strip_suffix("_H"))
            .unwrap_or(guard);
        if !stem.is_empty() {
            return Some(stem.to_lowercase().replace('_', "-"));
        }
    }

    for cap in TYPEDEF_RE.captures_iter(content).take(3) {
        symbols.push(Sym { name: cap[1].to_string(), priority: 8 });
    }
    for cap in TYPEDEF_SIMPLE_RE.captures_iter(content).take(2) {
        symbols.push(Sym { name: cap[1].to_string(), priority: 8 });
    }
    for cap in ENUM_RE.captures_iter(content).take(2) {
        symbols.push(Sym { name: cap[1].to_string(), priority: 8 });
    }
    for cap in STRUCT_RE.captures_iter(content).take(2) {
        symbols.push(Sym { name: cap[1].to_string(), priority: 9 });
    }
    for cap in DEFINE_RE.captures_iter(content).take(3) {
        let name = &cap[1];
        if !NOISE.contains(&name) && !name.ends_with("_H") {
            symbols.push(Sym { name: name.to_string(), priority: 6 });
        }
    }
    for cap in FUNC_DECL_RE.captures_iter(content).take(3) {
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

/// Shared C/C++ tree-sitter collector.
pub(super) fn collect_c_cpp(
    root: &tree_sitter::Node,
    src: &[u8],
    symbols: &mut Vec<Symbol>,
) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "namespace_definition" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 10 });
                }
            }
            "class_specifier" | "struct_specifier" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 9 });
                }
            }
            "enum_specifier" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 8 });
                }
            }
            "type_definition" => {
                if let Some(decl) = child.child_by_field_name("declarator") {
                    if let Some(name) = extract_identifier_from_declarator(&decl, src) {
                        symbols.push(Symbol { name, priority: 8 });
                    }
                }
            }
            "function_definition" | "declaration" => {
                if let Some(decl) = child.child_by_field_name("declarator") {
                    if let Some(name) = extract_identifier_from_declarator(&decl, src) {
                        symbols.push(Symbol { name, priority: 7 });
                    }
                }
            }
            "template_declaration" => {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    match inner.kind() {
                        "class_specifier" | "struct_specifier" => {
                            if let Some(name) = field_text(&inner, "name", src) {
                                symbols.push(Symbol { name: name.to_string(), priority: 9 });
                            }
                        }
                        "function_definition" => {
                            if let Some(decl) = inner.child_by_field_name("declarator") {
                                if let Some(name) = extract_identifier_from_declarator(&decl, src) {
                                    symbols.push(Symbol { name, priority: 7 });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_c(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn header_guard() {
        let src = "#ifndef MY_UTILS_H\n#define MY_UTILS_H\nvoid do_stuff();\n#endif";
        let n = name(src).unwrap();
        assert!(n.contains("my-utils"), "got: {n}");
    }

    #[test]
    fn typedef_struct() {
        let src = "typedef struct {\n    int x;\n    int y;\n} Point;\n";
        let n = name(src).unwrap();
        assert!(n.contains("point"), "got: {n}");
    }

    #[test]
    fn struct_and_function() {
        let src = "struct HashTable {\n    int size;\n    void **entries;\n};\nint hash_insert(struct HashTable *ht, const char *key, void *value) { return 0; }";
        let n = name(src).unwrap();
        assert!(n.contains("hash-table") || n.contains("hash-insert"), "got: {n}");
    }
}
