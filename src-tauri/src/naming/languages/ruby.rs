use std::collections::HashSet;

use super::NamingDefinition;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "ruby",
        extension: "rb",
        extract: extract_ruby,
    }
}

/// Ruby extraction with module/class awareness and DSL detection.
///
/// Priority order:
///   1. `module` declarations — P10
///   2. `class` declarations — P9
///   3. `def` methods — P7
///   4. DSL patterns: Rails migrations, Gemspec, Rake tasks — P10
fn extract_ruby(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    // DSL: Gemspec
    static GEMSPEC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)Gem::Specification\.new\b.*?\|.*?\|\s*\n\s*\w+\.name\s*=\s*["']([^"']+)["']"#).unwrap()
    });
    // DSL: Rails migration
    static MIGRATION_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^class\s+(\w+)\s*<\s*ActiveRecord::Migration").unwrap()
    });
    // DSL: Rake task
    static RAKE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^(?:desc|task)\s+["':]([\w:]+)"#).unwrap()
    });

    static MODULE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^[ \t]*module\s+([A-Z][a-zA-Z0-9_:]*)").unwrap());
    static CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^[ \t]*class\s+([A-Z][a-zA-Z0-9_:]*)").unwrap()
    });
    static DEF_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^[ \t]*def\s+(self\.)?([a-zA-Z_][a-zA-Z0-9_?!]*)").unwrap()
    });

    const NOISE: &[&str] = &[
        "initialize", "setup", "run", "call", "to_s", "to_h", "to_a",
        "inspect", "hash", "eql?", "test", "self",
    ];

    struct Symbol { name: String, priority: u8 }
    let mut symbols: Vec<Symbol> = Vec::new();

    // DSL patterns (highest priority)
    if let Some(cap) = GEMSPEC_RE.captures(content) {
        return Some(cap[1].to_string());
    }
    if let Some(cap) = MIGRATION_RE.captures(content) {
        return Some(cap[1].to_string());
    }
    if let Some(cap) = RAKE_RE.captures(content) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 10 });
    }

    // Modules (P10)
    for cap in MODULE_RE.captures_iter(content).take(3) {
        let name = &cap[1];
        // Take last segment of nested modules: Foo::Bar → Bar
        let short = name.rsplit("::").next().unwrap_or(name);
        if !short.is_empty() {
            symbols.push(Symbol { name: short.to_string(), priority: 10 });
        }
    }

    // Classes (P9)
    for cap in CLASS_RE.captures_iter(content).take(3) {
        let name = &cap[1];
        let short = name.rsplit("::").next().unwrap_or(name);
        if !short.is_empty() {
            symbols.push(Symbol { name: short.to_string(), priority: 9 });
        }
    }

    // Methods (P7)
    for cap in DEF_RE.captures_iter(content).take(4) {
        let name = cap[2].to_string();
        if !NOISE.contains(&name.as_str()) {
            symbols.push(Symbol { name, priority: 7 });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_ruby(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn module_and_class() {
        let src = "module Authentication\n  class SessionManager\n    def create_session\n    end\n  end\nend";
        let n = name(src).unwrap();
        assert!(n.contains("authentication"), "got: {n}");
    }

    #[test]
    fn migration() {
        let src = "class CreateUsers < ActiveRecord::Migration[7.0]\n  def change\n    create_table :users do |t|\n    end\n  end\nend";
        let n = name(src).unwrap();
        assert!(n.contains("create-users"), "got: {n}");
    }

    #[test]
    fn top_level_def() {
        let src = "def fibonacci(n)\n  return n if n <= 1\n  fibonacci(n-1) + fibonacci(n-2)\nend";
        let n = name(src).unwrap();
        assert!(n.contains("fibonacci"), "got: {n}");
    }
}

