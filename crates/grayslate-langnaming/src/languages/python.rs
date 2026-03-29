use super::{NamingDefinition, Extractor};
use crate::code::{is_noise_name, symbols_to_stem, Symbol};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "python",
        extension: "py",
        extract: Extractor::Custom(extract_python),
    }
}

fn extract_python(content: &str) -> Option<String> {
    extract_python_regex(content)
}

/// Regex-based Python naming: class, def, __all__, from-import, docstring.
fn extract_python_regex(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:@\w+\s*\n)*class\s+([A-Za-z_]\w*)\s*[\(:]").unwrap()
    });
    static FUNC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:async\s+)?def\s+([a-zA-Z][a-zA-Z0-9_]*)\s*\(").unwrap()
    });
    static ALL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^__all__\s*=\s*\[([^\]]+)\]"#).unwrap()
    });
    static ALL_ITEM_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"["']([A-Za-z_]\w*)["']"#).unwrap()
    });
    static FROM_IMPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^from\s+([\w.]+)\s+import").unwrap()
    });
    // Triple-quoted docstring at module level (first non-comment, non-import line)
    static DOCSTRING_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?s)\A(?:\s*#[^\n]*\n)*\s*(?:"""([^"]{5,80})(?:"""|\.\.\.)|'''([^']{5,80})(?:'''|\.\.\.))"#).unwrap()
    });

    let mut symbols: Vec<Symbol> = Vec::new();

    // __all__ — high priority
    if let Some(all_cap) = ALL_RE.captures(content) {
        let all_content = &all_cap[1];
        for item in ALL_ITEM_RE.captures_iter(all_content).take(3) {
            let name = item[1].to_string();
            if !is_noise_name(&name) {
                symbols.push(Symbol { name, priority: 8 });
            }
        }
    }

    // Classes
    for cap in CLASS_RE.captures_iter(content).take(3) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 9 });
    }

    // Functions (non-private)
    for cap in FUNC_RE.captures_iter(content).take(3) {
        let name = &cap[1];
        if !name.starts_with('_') {
            symbols.push(Symbol { name: name.to_string(), priority: 7 });
        }
    }

    if let Some(stem) = symbols_to_stem(&mut symbols) {
        return Some(stem);
    }

    // Fallback: significant imports
    for cap in FROM_IMPORT_RE.captures_iter(content).take(2) {
        let module = &cap[1];
        let short = module.rsplit('.').next().unwrap_or(module);
        if !short.is_empty() && !is_noise_name(short) && short.len() > 2 {
            symbols.push(Symbol { name: short.to_string(), priority: 4 });
        }
    }

    if let Some(stem) = symbols_to_stem(&mut symbols) {
        return Some(stem);
    }

    // Final fallback: module docstring
    if let Some(cap) = DOCSTRING_RE.captures(content) {
        let text = cap.get(1).or(cap.get(2)).map(|m| m.as_str().trim());
        if let Some(t) = text {
            let first_line = t.lines().next().unwrap_or("").trim();
            if first_line.len() >= 5 && first_line.len() <= 80 {
                return Some(first_line.to_string());
            }
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

    #[test]
    fn async_function() {
        let code = "import asyncio\n\nasync def fetch_user_data(user_id: int):\n    return await db.get(user_id)\n";
        let result = extract_python(code).unwrap();
        assert!(result.contains("fetch_user_data"), "async def captured: {result}");
    }

    #[test]
    fn class_beats_function() {
        let code = "def helper(): pass\n\nclass DatabaseMigration:\n    def up(self): pass\n";
        let result = extract_python(code).unwrap();
        assert!(result.contains("DatabaseMigration"), "class P9 > def P7: {result}");
    }
}
