use super::NamingDefinition;
use crate::naming::code::{
    extract_with_regex, extract_with_tree_sitter, field_text, has_pub_child, Symbol,
};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "rust",
        extension: "rs",
        extract: extract_rust,
    }
}

fn extract_rust(content: &str) -> Option<String> {
    // Try tree-sitter first; fall back to regex for truncated / unparseable content.
    extract_with_tree_sitter(content, tree_sitter_rust::LANGUAGE, collect_rust)
        .or_else(|| extract_rust_regex(content))
}

fn collect_rust(root: &tree_sitter::Node, src: &[u8], symbols: &mut Vec<Symbol>) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "mod_item" => {
                if let Some(name) = field_text(&child, "name", src) {
                    let pri = if has_pub_child(&child, src) { 10 } else { 8 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            "struct_item" => {
                if let Some(name) = field_text(&child, "name", src) {
                    let pri = if has_pub_child(&child, src) { 9 } else { 6 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            "enum_item" => {
                if let Some(name) = field_text(&child, "name", src) {
                    let pri = if has_pub_child(&child, src) { 9 } else { 6 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            "trait_item" => {
                if let Some(name) = field_text(&child, "name", src) {
                    let pri = if has_pub_child(&child, src) { 9 } else { 6 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            "impl_item" => {
                if let Some(type_node) = child.child_by_field_name("type") {
                    if let Ok(name) = type_node.utf8_text(src) {
                        let clean = name.split('<').next().unwrap_or(name).trim();
                        if !clean.is_empty() {
                            symbols.push(Symbol { name: clean.to_string(), priority: 5 });
                        }
                    }
                }
            }
            "function_item" => {
                if let Some(name) = field_text(&child, "name", src) {
                    let pri = if has_pub_child(&child, src) { 7 } else { 5 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            _ => {}
        }
    }

    // Fallback: extract `//!` module doc comments when no real symbols found
    if symbols.is_empty() {
        if let Some(desc) = extract_module_doc(src) {
            symbols.push(Symbol { name: desc, priority: 3 });
        }
    }
}

/// Regex fallback for Rust files where tree-sitter couldn't parse (e.g.
/// truncated content). Extracts `pub fn`, `pub struct`, `fn` patterns.
fn extract_rust_regex(content: &str) -> Option<String> {
    let patterns: &[(&str, u8)] = &[
        (r"(?m)^\s*pub\s+struct\s+([A-Z][A-Za-z0-9_]*)", 9),
        (r"(?m)^\s*pub\s+enum\s+([A-Z][A-Za-z0-9_]*)", 9),
        (r"(?m)^\s*pub\s+trait\s+([A-Z][A-Za-z0-9_]*)", 9),
        (r"(?m)^\s*pub\s+fn\s+([a-zA-Z_]\w*)", 7),
        (r"(?m)^fn\s+([a-zA-Z_]\w*)", 5),
    ];
    extract_with_regex(content, patterns)
}

/// Extract the first `//!` module-level doc comment line as a fallback name.
fn extract_module_doc(src: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(src).ok()?;
    for line in text.lines().take(20) {
        let trimmed = line.trim();
        if trimmed.starts_with("//!") {
            let comment = trimmed.trim_start_matches("//!").trim();
            if comment.len() >= 5 && comment.len() <= 80 {
                return Some(comment.to_string());
            }
        } else if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with('#') {
            break; // past the doc comment header
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pub_struct_and_mod() {
        let code = "pub mod authentication;\n\npub struct TokenParser {\n    inner: Vec<u8>,\n}\n\npub fn parse(input: &str) -> Token { todo!() }";
        let result = extract_rust(code).unwrap();
        assert!(result.starts_with("authentication"), "pub mod first: {result}");
    }

    #[test]
    fn filters_main() {
        let code = "pub struct Config { host: String }\nfn main() { let cfg = Config::from_env(); }";
        let result = extract_rust(code).unwrap();
        assert!(result.contains("Config"), "got: {result}");
        assert!(!result.contains("main"), "main filtered: {result}");
    }

    #[test]
    fn module_doc_comment_fallback() {
        let code = "//! HTTP client connection pooling\n\nuse std::collections::HashMap;\n";
        let result = extract_rust(code).unwrap();
        assert!(result.contains("HTTP client connection pooling"), "doc comment: {result}");
    }

    #[test]
    fn mod_tests_filtered_picks_function() {
        // A typical Rust file with `mod tests` at top-level — should not produce "tests".
        let code = r#"
pub fn detect_by_scoring(content: &str) -> Option<String> {
    todo!()
}

fn helper() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {}
}
"#;
        let result = extract_rust(code).unwrap();
        assert!(!result.contains("tests"), "mod tests should be noise-filtered: {result}");
        assert!(result.contains("detect"), "should pick the pub fn: {result}");
    }

    #[test]
    fn private_fn_when_only_mod_tests() {
        // File with only a private function and mod tests — should pick the function.
        let code = "fn process_chunk(data: &[u8]) -> Vec<u8> { vec![] }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn it_works() {}\n}";
        let result = extract_rust(code).unwrap();
        assert!(result.contains("process_chunk"), "got: {result}");
    }

    #[test]
    fn only_definition_falls_to_regex() {
        // File where tree-sitter only finds `definition()` (noise) → regex picks next fn.
        let code = "\
use super::NamingDefinition;

pub fn definition() -> NamingDefinition {
    NamingDefinition { name: \"c\", extension: \"c\", extract: extract_c }
}

fn extract_c(content: &str) -> Option<String> {
    None
}
";
        let result = extract_rust(code).unwrap();
        assert!(!result.contains("definition"), "definition is noise: {result}");
        assert!(result.contains("extract"), "regex fallback finds extract_c: {result}");
    }

    #[test]
    fn truncated_content_uses_regex() {
        // Simulate truncated content: a function whose body is cut off.
        let code = "\
use super::NamingDefinition;

pub fn definition() -> NamingDefinition {
    NamingDefinition { name: \"yaml\", extension: \"yaml\", extract: my_extractor }
}

fn my_extractor(content: &str) -> Option<String> {
    let x = 1;
    // ... body continues but is truncated
";
        let result = extract_rust(code).unwrap();
        assert!(result.contains("my"), "regex fallback: {result}");
    }
}
