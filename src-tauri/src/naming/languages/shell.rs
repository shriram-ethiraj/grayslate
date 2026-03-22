use std::collections::HashSet;

use super::NamingDefinition;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "shell",
        extension: "sh",
        extract: extract_shell,
    }
}

/// Shell script naming with comment and function extraction.
///
/// Priority order:
///   1. Description comment (first `#` comment after shebang that isn't boilerplate) — P10
///   2. Function names — P7
///   3. Key variable assignments (SCREAMING_CASE) — P5
fn extract_shell(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static FUNC_BASH_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^([a-zA-Z_][a-zA-Z0-9_]*)\s*\(\s*\)").unwrap());
    static FUNC_KW_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^function\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap());
    static VAR_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^([A-Z][A-Z0-9_]{2,})=").unwrap());

    const NOISE_FUNCS: &[&str] = &[
        "main", "init", "setup", "run", "start", "cleanup", "usage", "help",
        "die", "warn", "log", "error", "debug", "info", "exit_handler",
    ];
    const NOISE_VARS: &[&str] = &[
        "PATH", "HOME", "USER", "PWD", "SHELL", "TERM", "LANG", "LC_ALL",
        "VERBOSE", "DEBUG", "QUIET", "DRY_RUN", "FORCE",
    ];

    struct Symbol { name: String, priority: u8 }
    let mut symbols: Vec<Symbol> = Vec::new();

    // Try to extract a description from early comments.
    let mut past_shebang = false;
    for line in content.lines().take(15) {
        let trimmed = line.trim();
        if trimmed.starts_with("#!") {
            past_shebang = true;
            continue;
        }
        if trimmed.is_empty() { continue; }
        if trimmed.starts_with('#') {
            let comment = trimmed.trim_start_matches('#').trim();
            // Skip boilerplate: license, shellcheck, vim modelines, empty comments
            if comment.is_empty()
                || comment.starts_with('!')
                || comment.to_lowercase().starts_with("shellcheck")
                || comment.to_lowercase().starts_with("vim:")
                || comment.to_lowercase().starts_with("copyright")
                || comment.to_lowercase().starts_with("license")
                || comment.to_lowercase().starts_with("author")
                || comment.starts_with('-')
                || comment.starts_with('=')
            {
                continue;
            }
            if past_shebang && comment.len() >= 5 && comment.len() <= 80 {
                symbols.push(Symbol { name: comment.to_string(), priority: 10 });
                break;
            }
        }
        if !trimmed.starts_with('#') {
            break; // Past the header comments
        }
    }

    // Functions
    for cap in FUNC_BASH_RE.captures_iter(content).take(4) {
        let name = cap[1].to_string();
        if !NOISE_FUNCS.contains(&name.as_str()) {
            symbols.push(Symbol { name, priority: 7 });
        }
    }
    for cap in FUNC_KW_RE.captures_iter(content).take(4) {
        let name = cap[1].to_string();
        if !NOISE_FUNCS.contains(&name.as_str()) {
            symbols.push(Symbol { name, priority: 7 });
        }
    }

    // Key variables
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
        extract_shell(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn description_comment() {
        let src = "#!/bin/bash\n# Deploy the application to production\nset -e\n";
        let n = name(src).unwrap();
        assert!(n.contains("deploy"), "got: {n}");
    }

    #[test]
    fn function_names() {
        let src = "#!/bin/sh\n\nbuild_project() {\n  make all\n}\n\nrun_tests() {\n  make test\n}\n";
        let n = name(src).unwrap();
        assert!(n.contains("build-project"), "got: {n}");
    }

    #[test]
    fn key_variables() {
        let src = "#!/bin/bash\nPROJECT_NAME=\"myapp\"\nBUILD_DIR=\"./build\"\n";
        let n = name(src).unwrap();
        assert!(n.contains("project-name"), "got: {n}");
    }
}

