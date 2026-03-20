/// Phase 2 — Shebang line detection.
///
/// Parses `#!/...` lines to identify the interpreter.
use regex::Regex;
use std::sync::LazyLock;

struct ShebangPattern {
    pattern: &'static str,
    language: &'static str,
}

static SHEBANG_PATTERNS: &[ShebangPattern] = &[
    ShebangPattern {
        pattern: r"\bpython[23w]?\b",
        language: "python",
    },
    ShebangPattern {
        pattern: r"\bnode(js)?\b",
        language: "javascript",
    },
    ShebangPattern {
        pattern: r"\bdeno\b",
        language: "typescript",
    },
    ShebangPattern {
        pattern: r"\b(ba|z|k|fi)?sh\b",
        language: "shell",
    },
    ShebangPattern {
        pattern: r"\bperl\b",
        language: "text",
    },
    ShebangPattern {
        pattern: r"\bruby\b",
        language: "ruby",
    },
    ShebangPattern {
        pattern: r"\bphp\b",
        language: "php",
    },
];

static COMPILED_SHEBANGS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    SHEBANG_PATTERNS
        .iter()
        .map(|sp| (Regex::new(sp.pattern).unwrap(), sp.language))
        .collect()
});

/// Detect language from a shebang line (e.g. `#!/usr/bin/env python3`).
///
/// The input should be the first line of the content, already confirmed
/// to start with `#!`.
pub fn detect_by_shebang(first_line: &str) -> Option<&'static str> {
    for (regex, language) in COMPILED_SHEBANGS.iter() {
        if regex.is_match(first_line) {
            return Some(language);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_shebang() {
        assert_eq!(detect_by_shebang("#!/usr/bin/env python3"), Some("python"));
        assert_eq!(detect_by_shebang("#!/usr/bin/python"), Some("python"));
    }

    #[test]
    fn node_shebang() {
        assert_eq!(detect_by_shebang("#!/usr/bin/env node"), Some("javascript"));
        assert_eq!(detect_by_shebang("#!/usr/bin/nodejs"), Some("javascript"));
    }

    #[test]
    fn deno_shebang() {
        assert_eq!(
            detect_by_shebang("#!/usr/bin/env deno"),
            Some("typescript")
        );
    }

    #[test]
    fn shell_shebang() {
        assert_eq!(detect_by_shebang("#!/bin/bash"), Some("shell"));
        assert_eq!(detect_by_shebang("#!/bin/sh"), Some("shell"));
        assert_eq!(detect_by_shebang("#!/usr/bin/env zsh"), Some("shell"));
    }

    #[test]
    fn ruby_shebang() {
        assert_eq!(detect_by_shebang("#!/usr/bin/env ruby"), Some("ruby"));
    }

    #[test]
    fn unknown_shebang() {
        assert_eq!(detect_by_shebang("#!/usr/bin/env unknown"), None);
    }
}
