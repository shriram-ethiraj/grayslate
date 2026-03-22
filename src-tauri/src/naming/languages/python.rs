use super::NamingDefinition;
use crate::naming::code::{extract_with_tree_sitter, field_text, is_noise_name, Symbol};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "python",
        extension: "py",
        extract: extract_python,
    }
}

fn extract_python(content: &str) -> Option<String> {
    extract_with_tree_sitter(content, tree_sitter_python::LANGUAGE, collect_python)
}

fn collect_python(root: &tree_sitter::Node, src: &[u8], symbols: &mut Vec<Symbol>) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "class_definition" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 9 });
                }
            }
            "function_definition" => {
                if let Some(name) = field_text(&child, "name", src) {
                    if !name.starts_with('_') {
                        symbols.push(Symbol { name: name.to_string(), priority: 7 });
                    }
                }
            }
            "decorated_definition" => {
                if let Some(inner) = child.child_by_field_name("definition") {
                    match inner.kind() {
                        "class_definition" => {
                            if let Some(name) = field_text(&inner, "name", src) {
                                symbols.push(Symbol { name: name.to_string(), priority: 9 });
                            }
                        }
                        "function_definition" => {
                            if let Some(name) = field_text(&inner, "name", src) {
                                if !name.starts_with('_') {
                                    symbols.push(Symbol { name: name.to_string(), priority: 7 });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            // `__all__ = ["Foo", "Bar"]` — exported names, high priority
            "expression_statement" => {
                collect_dunder_all(&child, src, symbols);
            }
            _ => {}
        }
    }

    // If no class/function/all was found, try to extract from imports
    // (e.g., `from sklearn.ensemble import RandomForestClassifier`)
    if symbols.iter().all(|s| is_noise_name(&s.name)) || symbols.is_empty() {
        collect_significant_imports(root, src, symbols);
    }

    // Final fallback: module docstring (first string literal at module level)
    if symbols.is_empty() {
        if let Some(desc) = extract_module_docstring(root, src) {
            symbols.push(Symbol { name: desc, priority: 3 });
        }
    }
}

/// Extract names from `__all__ = ["Foo", "Bar"]`.
fn collect_dunder_all(node: &tree_sitter::Node, src: &[u8], symbols: &mut Vec<Symbol>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "assignment" {
            if let Some(left) = child.child_by_field_name("left") {
                if left.utf8_text(src).ok() == Some("__all__") {
                    if let Some(right) = child.child_by_field_name("right") {
                        if right.kind() == "list" {
                            let mut inner = right.walk();
                            for elem in right.children(&mut inner) {
                                if elem.kind() == "string" {
                                    if let Ok(text) = elem.utf8_text(src) {
                                        let name = text.trim_matches(|c| c == '\'' || c == '"');
                                        if !name.is_empty() && !is_noise_name(name) {
                                            symbols.push(Symbol {
                                                name: name.to_string(),
                                                priority: 8,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// For scripts without classes/functions, extract meaningful import targets.
fn collect_significant_imports(root: &tree_sitter::Node, src: &[u8], symbols: &mut Vec<Symbol>) {
    use regex::Regex;
    use std::sync::LazyLock;

    static FROM_IMPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^from\s+([\w.]+)\s+import").unwrap()
    });

    let text = root.utf8_text(src).unwrap_or("");
    for cap in FROM_IMPORT_RE.captures_iter(text).take(2) {
        let module = &cap[1];
        // Take the last meaningful segment: `flask.views` → "views"
        let short = module.rsplit('.').next().unwrap_or(module);
        if !short.is_empty() && !is_noise_name(short) && short.len() > 2 {
            symbols.push(Symbol { name: short.to_string(), priority: 4 });
        }
    }
}

/// Extract module-level docstring (first expression_statement that is a string).
fn extract_module_docstring(root: &tree_sitter::Node, src: &[u8]) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "expression_statement" => {
                let mut inner = child.walk();
                for gc in child.children(&mut inner) {
                    if gc.kind() == "string" || gc.kind() == "concatenated_string" {
                        if let Ok(text) = gc.utf8_text(src) {
                            let clean = text
                                .trim_matches(|c| c == '"' || c == '\'')
                                .trim();
                            // Take only the first line of the docstring
                            let first_line = clean.lines().next().unwrap_or("").trim();
                            if first_line.len() >= 5 && first_line.len() <= 80 {
                                return Some(first_line.to_string());
                            }
                        }
                    }
                }
                // Only check the very first expression_statement
                return None;
            }
            // Skip past imports, comments, and future annotations
            "import_statement" | "import_from_statement" | "comment" | "future_import_statement" => continue,
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_and_toplevel_fn() {
        let code = "class UserAuthentication:\n    def login(self): pass\n\ndef setup_logging(): pass";
        let result = extract_python(code).unwrap();
        assert!(result.contains("UserAuthentication"), "got: {result}");
        assert!(!result.contains("login"), "methods should not appear: {result}");
    }

    #[test]
    fn decorated_class() {
        let code = "@dataclass\nclass Config:\n    host: str\n    port: int";
        let result = extract_python(code).unwrap();
        assert!(result.contains("Config"), "got: {result}");
    }

    #[test]
    fn dunder_all_extraction() {
        let code = "__all__ = [\"TokenParser\", \"TokenValidator\"]\n\nclass TokenParser:\n    pass\n\nclass TokenValidator:\n    pass";
        let result = extract_python(code).unwrap();
        assert!(result.contains("TokenParser"), "got: {result}");
    }

    #[test]
    fn private_functions_excluded() {
        let code = "def _helper(): pass\ndef _internal(): pass\ndef process_data(): pass";
        let result = extract_python(code).unwrap();
        assert!(result.contains("process_data"), "got: {result}");
        assert!(!result.contains("helper"), "private excluded: {result}");
    }

    #[test]
    fn script_with_imports() {
        let code = "from sklearn.ensemble import RandomForestClassifier\nimport numpy as np\n\nX = np.array([1,2,3])";
        let result = extract_python(code).unwrap();
        assert!(result.contains("ensemble"), "import fallback: {result}");
    }

    #[test]
    fn module_docstring_fallback() {
        let code = "\"\"\"HTTP request rate limiter with token bucket algorithm\"\"\"\n\nimport time\n";
        let result = extract_python(code).unwrap();
        assert!(result.contains("HTTP request rate limiter"), "docstring: {result}");
    }
}
