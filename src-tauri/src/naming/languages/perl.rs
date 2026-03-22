use std::collections::HashSet;

use super::NamingDefinition;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "perl",
        extension: "pl",
        extract: extract_perl,
    }
}

/// Perl naming extraction.
///
/// Priority order:
///   1. `package` declaration — P10 (uses last 2 segments for richer context)
///   2. `sub` declarations — P7
///   3. `use` module imports — P5
fn extract_perl(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static PACKAGE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^package\s+([\w:]+)").unwrap());
    static SUB_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^sub\s+([a-zA-Z_]\w*)").unwrap());
    static USE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^use\s+([A-Z][\w:]+)").unwrap());
    // POD =head1 NAME section
    static POD_NAME_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^=head1\s+NAME\s*\n+\s*(\S.+)$").unwrap());

    const NOISE: &[&str] = &[
        "main", "new", "init", "import", "AUTOLOAD", "DESTROY",
        "BEGIN", "END", "strict", "warnings", "utf8", "vars",
        "Exporter", "Carp", "Data",
    ];

    struct Symbol { name: String, priority: u8 }
    let mut symbols: Vec<Symbol> = Vec::new();

    // POD =head1 NAME — highest quality naming signal
    if let Some(cap) = POD_NAME_RE.captures(content) {
        let name_line = cap[1].trim();
        // Often formatted as "Module::Name - description"
        let name = name_line.split(" - ").next().unwrap_or(name_line).trim();
        if !name.is_empty() && name.len() <= 60 {
            return Some(name.replace("::", "-"));
        }
    }

    // Package declaration → Foo::Bar::Baz → "Bar-Baz" (last 2 segments)
    if let Some(cap) = PACKAGE_RE.captures(content) {
        let full = &cap[1];
        let segments: Vec<&str> = full.split("::").collect();
        let short = if segments.len() >= 2 {
            format!("{}-{}", segments[segments.len() - 2], segments[segments.len() - 1])
        } else {
            segments.last().unwrap_or(&full).to_string()
        };
        if !short.is_empty() && !NOISE.contains(&short.as_str()) {
            symbols.push(Symbol { name: short, priority: 10 });
        }
    }

    // Subroutines (P7)
    for cap in SUB_RE.captures_iter(content).take(4) {
        let name = cap[1].to_string();
        if !NOISE.contains(&name.as_str()) && !name.starts_with('_') {
            symbols.push(Symbol { name, priority: 7 });
        }
    }

    // use modules (P5) — last segment only
    for cap in USE_RE.captures_iter(content).take(3) {
        let full = &cap[1];
        let short = full.rsplit("::").next().unwrap_or(full);
        if !NOISE.contains(&short) {
            symbols.push(Symbol { name: short.to_string(), priority: 5 });
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
        extract_perl(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn package_preserves_hierarchy() {
        let src = "package MyApp::Auth;\nuse strict;\n\nsub authenticate {\n    my ($self, $user) = @_;\n}\n\nsub authorize {\n}";
        let n = name(src).unwrap();
        // With MAX_TOKENS=1, only the package token is kept
        assert!(n.contains("my-app-auth"), "got: {n}");
    }

    #[test]
    fn deep_package_uses_last_two() {
        let src = "package Com::Example::API::Router;\n\nsub dispatch { }";
        let n = name(src).unwrap();
        assert!(n.contains("api-router"), "last 2 segments: {n}");
    }

    #[test]
    fn pod_name_extraction() {
        let src = "=head1 NAME\n\nText::CSV - comma-separated values manipulator\n\n=head1 SYNOPSIS\n\npackage Text::CSV;\nsub parse { }";
        let n = name(src).unwrap();
        assert!(n.contains("text-csv"), "POD name: {n}");
    }

    #[test]
    fn script_with_subs() {
        let src = "#!/usr/bin/perl\nuse File::Path;\n\nsub process_file {\n    my $file = shift;\n}\n";
        let n = name(src).unwrap();
        assert!(n.contains("process-file"), "got: {n}");
    }

    #[test]
    fn private_subs_excluded() {
        let src = "package Foo;\nsub _internal { }\nsub public_api { }";
        let n = name(src).unwrap();
        assert!(!n.contains("internal"), "private excluded: {n}");
        assert!(n.contains("foo"), "package kept: {n}");
    }
}

