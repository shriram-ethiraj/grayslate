use std::collections::HashSet;

use super::model::MAX_TOKENS;

// ---------------------------------------------------------------------------
// Shared code-extraction utilities.
//
// Language-specific logic lives in `languages/<lang>.rs`. This module provides
// reusable building blocks: Symbol, noise filtering, regex helpers, and the
// `symbols_to_stem` finaliser that every language extractor can call.
//
// Two extraction styles are supported:
//   1. **Declarative** (`Extractor::Patterns`): language defines a list of
//      `SymbolPattern`s + optional noise list. `extract_from_patterns()` handles
//      the rest (regex matching, priority sort, dedup, noise filter, stem join).
//   2. **Custom** (`Extractor::Custom`): language provides a function with
//      full control over extraction logic (multi-stage, known-pattern, etc.).
// ---------------------------------------------------------------------------

/// Noise symbol names that are too generic to be useful in a filename.
const NOISE_NAMES: &[&str] = &[
    "main", "init", "setup", "run", "start", "new", "default", "handle",
    "index", "app", "mod", "test", "tests", "self", "this", "cls",
    "definition",
];

/// A declarative regex-based symbol extraction pattern.
///
/// Used by `Extractor::Patterns` in `NamingDefinition`. Each pattern describes
/// a regex that captures a symbol name from source content, along with a
/// priority ranking (higher = more semantically meaningful for naming).
pub struct SymbolPattern {
    /// Regex pattern string (must have at least one capture group).
    pub regex: &'static str,
    /// Priority for ranking: 4-10 (higher = more meaningful).
    ///   10 – entry points, @main, primary exports
    ///    9 – types (class, struct, enum, trait)
    ///    8 – secondary types, protocols, mixins
    ///    7 – functions, methods
    ///    6 – extensions, aliases
    ///    5 – imports, packages, context
    ///    4 – variables, constants, fallback
    pub priority: u8,
    /// Which capture group to extract (usually 1).
    pub capture_group: usize,
}

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

/// Extract a stem from content using declarative `SymbolPattern` definitions.
///
/// This is the shared pipeline used by `Extractor::Patterns`. It:
///   1. Runs each pattern against content, collecting up to 3 captures per pattern
///   2. Filters noise names (global `NOISE_NAMES` + language-specific `extra_noise`)
///   3. Sorts by priority (descending)
///   4. Deduplicates and joins into a stem
pub(crate) fn extract_from_patterns(
    content: &str,
    patterns: &[SymbolPattern],
    extra_noise: &[&str],
) -> Option<String> {
    let mut symbols = Vec::new();

    for pat in patterns {
        if let Ok(re) = regex::Regex::new(pat.regex) {
            for cap in re.captures_iter(content).take(3) {
                if let Some(m) = cap.get(pat.capture_group) {
                    let name = m.as_str().to_string();
                    if !name.is_empty()
                        && !is_noise_name(&name)
                        && !extra_noise.contains(&name.as_str())
                    {
                        symbols.push(Symbol {
                            name,
                            priority: pat.priority,
                        });
                    }
                }
            }
        }
    }

    symbols_to_stem(&mut symbols)
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

    #[test]
    fn extract_from_patterns_priority_sort() {
        let content = "func helper() {}\nclass UserService {\n}";
        let patterns: &[SymbolPattern] = &[
            SymbolPattern { regex: r"(?m)^func\s+([a-zA-Z_]\w*)", priority: 7, capture_group: 1 },
            SymbolPattern { regex: r"(?m)^class\s+([A-Z]\w+)", priority: 9, capture_group: 1 },
        ];
        let result = extract_from_patterns(content, patterns, &[]).unwrap();
        // Class (P9) should beat func (P7)
        assert!(result.starts_with("UserService"), "priority sort: {result}");
    }

    #[test]
    fn extract_from_patterns_extra_noise() {
        let content = "class ViewController {}\nfunc configure() {}";
        let patterns: &[SymbolPattern] = &[
            SymbolPattern { regex: r"(?m)^class\s+([A-Z]\w+)", priority: 9, capture_group: 1 },
            SymbolPattern { regex: r"(?m)^func\s+([a-zA-Z_]\w*)", priority: 7, capture_group: 1 },
        ];
        let result = extract_from_patterns(content, patterns, &["configure"]).unwrap();
        assert!(!result.contains("configure"), "extra noise filtered: {result}");
        assert!(result.contains("ViewController"), "kept: {result}");
    }
}
