use std::collections::HashSet;

use super::NamingDefinition;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "cmd",
        extension: "bat",
        extract: extract_cmd,
    }
}

/// Windows Batch/CMD naming extraction.
///
/// Priority order:
///   1. First meaningful `REM` description comment (not boilerplate) — P10
///   2. Subroutine/jump labels (`:label`) — P7
///   3. Key `SET` variable assignments (SCREAMING_CASE, len > 3) — P5
fn extract_cmd(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static LABEL_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?im)^\:([a-zA-Z_][a-zA-Z0-9_]*)").unwrap());
    static VAR_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?im)^\s*set\s+([A-Z][A-Z0-9_]{2,})=").unwrap());

    const NOISE_LABELS: &[&str] = &[
        "end", "eof", "exit", "error", "err", "usage", "help",
        "start", "begin", "init", "main", "done",
    ];
    const NOISE_VARS: &[&str] = &[
        "PATH", "TEMP", "TMP", "USERPROFILE", "SYSTEMROOT", "WINDIR",
        "COMSPEC", "PROMPT", "ERRORLEVEL", "VERBOSE", "DEBUG", "CD",
    ];
    // Boilerplate prefixes to skip in REM comments
    const BOILERPLATE: &[&str] = &[
        "copyright", "license", "author", "version", "usage", "---", "===",
        "you may not use", "distributed on an", "unless required by",
        "without warranties", "all rights reserved", "permission is hereby granted",
    ];

    struct Symbol {
        name: String,
        priority: u8,
    }
    let mut symbols: Vec<Symbol> = Vec::new();

    // Extract the first meaningful REM comment (top 15 lines only).
    for line in content.lines().take(15) {
        let trimmed = line.trim();
        // Accept both `REM ...` and `:: ...` comment styles
        let comment = if trimmed.to_ascii_uppercase().starts_with("REM ") {
            trimmed[4..].trim()
        } else if trimmed.starts_with(":: ") || trimmed.starts_with("::") {
            trimmed[2..].trim()
        } else {
            continue;
        };

        if comment.is_empty() || comment.len() < 5 || comment.len() > 80 {
            continue;
        }
        let lower = comment.to_ascii_lowercase();
        if BOILERPLATE.iter().any(|p| lower.starts_with(p)) {
            continue;
        }
        symbols.push(Symbol { name: comment.to_string(), priority: 10 });
        break;
    }

    // Subroutine / jump labels
    for cap in LABEL_RE.captures_iter(content).take(4) {
        let name = cap[1].to_string();
        if !NOISE_LABELS.contains(&name.to_ascii_lowercase().as_str()) {
            symbols.push(Symbol { name, priority: 7 });
        }
    }

    // Key variable assignments
    for cap in VAR_RE.captures_iter(content).take(4) {
        let name = cap[1].to_string();
        if !NOISE_VARS.contains(&name.as_str()) {
            symbols.push(Symbol { name, priority: 5 });
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
        extract_cmd(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn rem_description() {
        let src = "@echo off\nREM Deploy the application to production\nsetlocal\n";
        let n = name(src).unwrap();
        assert!(n.contains("deploy"), "got: {n}");
    }

    #[test]
    fn double_colon_comment() {
        let src = "@echo off\n:: Build and package the release\nsetlocal\n";
        let n = name(src).unwrap();
        assert!(n.contains("build"), "got: {n}");
    }

    #[test]
    fn label_name() {
        let src = "@echo off\n:build_project\necho Building...\ngoto :eof\n";
        let n = name(src).unwrap();
        assert!(n.contains("build-project"), "got: {n}");
    }

    #[test]
    fn key_variable() {
        let src = "@echo off\nset PROJECT_NAME=myapp\nset BUILD_DIR=.\\build\n";
        let n = name(src).unwrap();
        assert!(n.contains("project-name"), "got: {n}");
    }

    #[test]
    fn noise_labels_excluded() {
        let src = "@echo off\n:end\necho Done.\n:eof\n";
        // No non-noise labels — falls back to None (no variable either)
        assert!(name(src).is_none(), "noise labels should not produce a name");
    }
}
