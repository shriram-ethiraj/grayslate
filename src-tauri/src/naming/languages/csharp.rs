use std::collections::HashSet;

use super::NamingDefinition;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "csharp",
        extension: "cs",
        extract: extract_csharp,
    }
}

/// C# regex extraction with namespace awareness.
///
/// Priority order:
///   1. `namespace` — P10
///   2. `class` / `interface` / `struct` / `enum` / `record` — P9
///   3. Public methods — P7
fn extract_csharp(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static NAMESPACE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^namespace\s+([\w.]+)").unwrap());
    static TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?m)^[ \t]*(?:(?:public|private|protected|internal|static|abstract|sealed|partial)\s+)*(?:class|interface|struct|enum|record)\s+([A-Z][a-zA-Z0-9_]*)",
        )
        .unwrap()
    });
    static METHOD_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?m)^[ \t]*(?:public|protected|internal)\s+(?:static\s+)?(?:async\s+)?(?:override\s+)?[\w<>\[\]?]+\s+([A-Z][a-zA-Z0-9_]*)\s*[(<]",
        )
        .unwrap()
    });

    const NOISE: &[&str] = &[
        "Main", "Program", "Startup", "App", "Test", "Tests",
        "ToString", "GetHashCode", "Equals", "Dispose", "Configure",
    ];

    struct Symbol { name: String, priority: u8 }
    let mut symbols: Vec<Symbol> = Vec::new();

    // Namespace (last segment)
    if let Some(cap) = NAMESPACE_RE.captures(content) {
        if let Some(ns) = cap[1].rsplit('.').next() {
            if !ns.is_empty() && !NOISE.contains(&ns) {
                symbols.push(Symbol { name: ns.to_string(), priority: 10 });
            }
        }
    }

    // Types (P9)
    for cap in TYPE_RE.captures_iter(content).take(4) {
        let name = cap[1].to_string();
        if !NOISE.contains(&name.as_str()) {
            symbols.push(Symbol { name, priority: 9 });
        }
    }

    // Public methods (P7)
    for cap in METHOD_RE.captures_iter(content).take(4) {
        let name = cap[1].to_string();
        if !NOISE.contains(&name.as_str()) {
            symbols.push(Symbol { name, priority: 7 });
        }
    }

    symbols.sort_by(|a, b| b.priority.cmp(&a.priority));

    let mut seen = HashSet::new();
    let mut tokens: Vec<String> = Vec::new();
    for sym in &symbols {
        if tokens.len() >= MAX_TOKENS {
            break;
        }
        if seen.insert(sym.name.clone()) {
            tokens.push(sym.name.clone());
        }
    }

    if tokens.is_empty() { None } else { Some(tokens.join("-")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_csharp(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn namespace_and_class() {
        let src = "namespace MyApp.Services\n{\n    public class UserService\n    {\n    }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("services"), "got: {n}");
    }

    #[test]
    fn interface() {
        let src = "public interface IRepository<T>\n{\n    Task<T> GetByIdAsync(int id);\n}";
        let n = name(src).unwrap();
        assert!(n.contains("irepository"), "got: {n}");
    }

    #[test]
    fn record_type() {
        let src = "namespace Models\n{\n    public record UserDto(string Name, int Age);\n}";
        let n = name(src).unwrap();
        assert!(n.contains("models"), "got: {n}");
    }
}

