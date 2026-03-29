use std::collections::HashSet;

use super::{NamingDefinition, Extractor};
use crate::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "csharp",
        extension: "cs",
        extract: Extractor::Custom(extract_csharp),
    }
}

/// C# regex extraction with namespace awareness.
///
/// Priority order (file-local types outrank namespace context):
///   1. `class` / `interface` / `struct` / `enum` / `record` — P9
///   2. Public methods — P7
///   3. `namespace` (last segment) — P5 (fallback context)
fn extract_csharp(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static NAMESPACE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^namespace\s+([\w.]+)").unwrap());
    static TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?m)^[ \t]*(?:(?:public|private|protected|internal|static|abstract|sealed|partial|readonly)\s+)*(?:class|interface|struct|enum|record)\s+([A-Z][a-zA-Z0-9_]*)",
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

    // Types (P9) — highest priority for file-local symbols
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

    // Namespace (last segment) — fallback context (P5)
    if let Some(cap) = NAMESPACE_RE.captures(content) {
        if let Some(ns) = cap[1].rsplit('.').next() {
            if !ns.is_empty() && !NOISE.contains(&ns) {
                symbols.push(Symbol { name: ns.to_string(), priority: 5 });
            }
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
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_csharp(src).and_then(|s| slugify(&s))
    }

    // --- Priority rebalance: type outranks namespace ---
    #[test]
    fn class_leads_over_namespace() {
        let src = "namespace MyApp.Services\n{\n    public class UserService\n    {\n    }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("user-service"), "class wins over namespace: {n}");
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
        assert!(n.contains("user-dto"), "record type wins over namespace: {n}");
    }

    // --- Audit regression: SqlMapper not Dapper ---
    #[test]
    fn class_wins_over_generic_namespace() {
        let src = "namespace Dapper\n{\n    public static partial class SqlMapper\n    {\n        public static int Execute() { return 0; }\n    }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("sql-mapper"), "class beats namespace: {n}");
    }

    #[test]
    fn command_definition_struct() {
        let src = "namespace Dapper\n{\n    public readonly struct CommandDefinition\n    {\n        public string CommandText { get; }\n    }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("command-definition"), "struct beats namespace: {n}");
    }

    // --- Namespace-only fallback ---
    #[test]
    fn namespace_only_when_no_types() {
        let src = "namespace MyApp.Utils\n{\n    // empty\n}";
        let n = name(src).unwrap();
        assert!(n.contains("utils"), "namespace fallback: {n}");
    }

    #[test]
    fn csharp_enum_type() {
        let src = "namespace Models\n{\n    public enum OrderStatus\n    {\n        Pending, Processing, Completed\n    }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("order-status"), "enum type: {n}");
    }

    #[test]
    fn csharp_abstract_class() {
        let src = "public abstract class BaseController\n{\n    public abstract void HandleRequest();\n}";
        let n = name(src).unwrap();
        assert!(n.contains("base-controller"), "abstract class: {n}");
    }

    #[test]
    fn csharp_multiple_types() {
        let src = "namespace Services\n{\n    public interface IOrderService\n    {\n        void PlaceOrder();\n    }\n\n    public class OrderService : IOrderService\n    {\n        public void PlaceOrder() { }\n    }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("order-service"), "multiple types: {n}");
    }
}

