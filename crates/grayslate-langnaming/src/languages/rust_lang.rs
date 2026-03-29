use super::{NamingDefinition, Extractor};
use crate::code::extract_with_regex;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "rust",
        extension: "rs",
        extract: Extractor::Custom(extract_rust),
    }
}

fn extract_rust(content: &str) -> Option<String> {
    extract_rust_regex(content)
        .or_else(|| extract_module_doc(content.as_bytes()))
}

/// Regex fallback for Rust files: pub mod, pub struct/enum/trait, pub fn, fn.
fn extract_rust_regex(content: &str) -> Option<String> {
    let patterns: &[(&str, u8)] = &[
        // pub mod has highest priority — it names the compilation unit
        (r"(?m)^\s*pub\s+mod\s+([a-zA-Z_]\w*)", 10),
        (r"(?m)^\s*pub(?:\([^)]+\))?\s+struct\s+([A-Z][A-Za-z0-9_]*)", 9),
        (r"(?m)^\s*pub(?:\([^)]+\))?\s+enum\s+([A-Z][A-Za-z0-9_]*)", 9),
        (r"(?m)^\s*pub(?:\([^)]+\))?\s+trait\s+([A-Z][A-Za-z0-9_]*)", 9),
        // macro_rules! — often the primary export of a crate module
        (r"(?m)^(?:#\[macro_export\]\s*\n\s*)?macro_rules!\s+([a-zA-Z_]\w*)", 8),
        // impl Type — provides context about the module's primary type
        (r"(?m)^impl(?:<[^>]*>)?\s+([A-Z][A-Za-z0-9_]*)", 7),
        // pub type alias
        (r"(?m)^\s*pub(?:\([^)]+\))?\s+type\s+([A-Z][A-Za-z0-9_]*)", 7),
        // pub const
        (r"(?m)^\s*pub(?:\([^)]+\))?\s+const\s+([A-Z][A-Z0-9_]{2,})", 6),
        (r"(?m)^\s*pub(?:\([^)]+\))?\s+fn\s+([a-zA-Z_]\w*)", 7),
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
            break;
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
        let code = "fn process_chunk(data: &[u8]) -> Vec<u8> { vec![] }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn it_works() {}\n}";
        let result = extract_rust(code).unwrap();
        assert!(result.contains("process_chunk"), "got: {result}");
    }

    #[test]
    fn only_definition_falls_to_regex() {
        let code = "\
use super::{NamingDefinition, Extractor};

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
        let code = "\
use super::{NamingDefinition, Extractor};

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

    #[test]
    fn pub_crate_struct() {
        let code = "pub(crate) struct ConfigManager {\n    inner: HashMap<String, Value>,\n}";
        let result = extract_rust(code).unwrap();
        assert!(result.contains("ConfigManager"), "pub(crate) struct: {result}");
    }

    #[test]
    fn impl_block() {
        let code = "impl TokenParser {\n    pub fn parse(&self, input: &str) -> Token { todo!() }\n}";
        let result = extract_rust(code).unwrap();
        assert!(result.contains("TokenParser"), "impl block: {result}");
    }

    #[test]
    fn macro_rules() {
        let code = "#[macro_export]\nmacro_rules! define_error {\n    ($name:ident) => { struct $name; };\n}";
        let result = extract_rust(code).unwrap();
        assert!(result.contains("define_error"), "macro_rules: {result}");
    }

    #[test]
    fn pub_enum() {
        let code = "pub enum TokenKind {\n    Identifier,\n    Literal,\n    Operator,\n}";
        let result = extract_rust(code).unwrap();
        assert!(result.contains("TokenKind"), "pub enum: {result}");
    }

    #[test]
    fn pub_trait() {
        let code = "pub trait Serializable {\n    fn serialize(&self) -> Vec<u8>;\n    fn deserialize(data: &[u8]) -> Self;\n}";
        let result = extract_rust(code).unwrap();
        assert!(result.contains("Serializable"), "pub trait: {result}");
    }
}
