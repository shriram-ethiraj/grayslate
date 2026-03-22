use std::collections::HashSet;

use super::NamingDefinition;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "powershell",
        extension: "ps1",
        extract: extract_powershell,
    }
}

/// PowerShell naming extraction.
///
/// Priority order:
///   1. Comment-based help: `.SYNOPSIS` / `.DESCRIPTION` — P10
///   2. `function` declarations (Verb-Noun) — P9
///   3. `param()` block parameter names — P5
fn extract_powershell(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static SYNOPSIS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?ims)\.SYNOPSIS\s*\n\s*(.{5,80})").unwrap()
    });
    static FUNC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?im)^function\s+([A-Za-z][\w-]*)").unwrap()
    });
    static PARAM_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)\$([A-Z][a-zA-Z0-9]+)").unwrap()
    });

    const NOISE: &[&str] = &[
        "Main", "Test", "Args", "ErrorActionPreference", "PSScriptRoot",
        "True", "False", "Null",
    ];

    struct Symbol { name: String, priority: u8 }
    let mut symbols: Vec<Symbol> = Vec::new();

    // .SYNOPSIS (P10)
    if let Some(cap) = SYNOPSIS_RE.captures(content) {
        let text = cap[1].trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }

    // Functions (P9)
    for cap in FUNC_RE.captures_iter(content).take(4) {
        let name = cap[1].to_string();
        if !NOISE.contains(&name.as_str()) {
            symbols.push(Symbol { name, priority: 9 });
        }
    }

    // Parameters (P5)
    let mut param_seen = HashSet::new();
    for cap in PARAM_RE.captures_iter(content).take(6) {
        let name = cap[1].to_string();
        if !NOISE.contains(&name.as_str()) && param_seen.insert(name.clone()) {
            symbols.push(Symbol { name, priority: 5 });
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
        extract_powershell(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn verb_noun_function() {
        let src = "function Get-UserProfile {\n    param($UserId)\n    # ...\n}";
        let n = name(src).unwrap();
        assert!(n.contains("get-user-profile"), "got: {n}");
    }

    #[test]
    fn synopsis() {
        let src = "<#\n.SYNOPSIS\n    Deploys the application to Azure\n.DESCRIPTION\n    Full deploy\n#>\nparam($Environment)\n";
        let n = name(src).unwrap();
        assert!(n.contains("deploys"), "got: {n}");
    }
}

