use super::NamingDefinition;
use crate::naming::code::extract_with_regex;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "php",
        extension: "php",
        extract: extract_php,
    }
}

fn extract_php(content: &str) -> Option<String> {
    const PATTERNS: &[(&str, u8)] = &[
        (r"(?m)^namespace\s+([A-Za-z_][\w\\]+)", 10),
        (r"(?m)^(?:abstract\s+|final\s+)?class\s+([A-Z]\w+)", 9),
        (r"(?m)^interface\s+([A-Z]\w+)", 9),
        (r"(?m)^trait\s+([A-Z]\w+)", 9),
        (r"(?m)^enum\s+([A-Z]\w+)", 8),
        (r"(?m)^function\s+([a-zA-Z_]\w+)\s*\(", 7),
    ];
    extract_with_regex(content, PATTERNS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_class_and_namespace() {
        let src = "<?php\nnamespace App\\Models;\n\nclass User extends Model {\n  public function name() {}\n}";
        let result = extract_php(src).unwrap();
        assert!(result.contains("App\\Models") || result.contains("User"), "got: {result}");
    }

    #[test]
    fn php_interface() {
        let src = "<?php\ninterface Cacheable {\n  public function getCacheKey(): string;\n}";
        assert!(extract_php(src).unwrap().contains("Cacheable"));
    }
}
