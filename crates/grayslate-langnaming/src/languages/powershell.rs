use std::collections::HashSet;

use super::{NamingDefinition, Extractor};
use crate::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "powershell",
        extension: "ps1",
        extract: Extractor::Custom(extract_powershell),
    }
}

/// PowerShell naming extraction.
///
/// Priority order:
///   1. Comment-based help: `.SYNOPSIS` — P10
///   2. Comment-based help: `.DESCRIPTION` (fallback) — P9
///   3. `function` declarations (Verb-Noun) — P9
///   4. `param()` block parameter names — P5
fn extract_powershell(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static SYNOPSIS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?ims)\.SYNOPSIS\s*\n\s*(.{5,80})").unwrap()
    });
    static DESCRIPTION_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?ims)\.DESCRIPTION\s*\n\s*(.{5,80})").unwrap()
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

    // .DESCRIPTION (P9 fallback when no SYNOPSIS)
    if let Some(cap) = DESCRIPTION_RE.captures(content) {
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
    use crate::shared::slugify;

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

    #[test]
    fn description_fallback() {
        let src = "<#\n.DESCRIPTION\n    Manages user certificates and renewals\n#>\nparam($CertPath)\n";
        let n = name(src).unwrap();
        assert!(n.contains("manages") || n.contains("certificates"), "description fallback: {n}");
    }

    #[test]
    fn multiple_functions() {
        let src = "function Import-UserData {\n    param($Path)\n}\n\nfunction Export-UserData {\n    param($Path)\n}\n";
        let n = name(src).unwrap();
        assert!(n.contains("import-user-data"), "first function wins: {n}");
    }

    #[test]
    fn param_only_fallback() {
        let src = "param(\n    [string]$ComputerName,\n    [int]$Port = 443\n)\nWrite-Host \"Connecting to $ComputerName:$Port\"\n";
        let n = name(src).unwrap();
        assert!(n.contains("computer-name"), "param fallback: {n}");
    }

    #[test]
    fn cmdlet_binding_function() {
        let src = "function Set-RegistryPermission {\n    [CmdletBinding()]\n    param(\n        [string]$KeyPath,\n        [string]$Identity\n    )\n    # ...\n}\n";
        let n = name(src).unwrap();
        assert!(n.contains("set-registry-permission"), "cmdlet function: {n}");
    }

    #[test]
    fn synopsis_beats_function() {
        let src = "<#\n.SYNOPSIS\n    Provision virtual machines in Azure\n#>\nfunction New-AzureVM {\n    param($Name)\n}\n";
        let n = name(src).unwrap();
        assert!(n.contains("provision"), "synopsis P10 > function P9: {n}");
    }
}

