/// Phase 2 — Shebang line detection.
///
/// Parses `#!/...` lines to identify the interpreter.
/// Patterns are auto-derived from per-language definitions in `languages/`.
use super::languages::SHEBANG_MAP;

/// Detect language from a shebang line (e.g. `#!/usr/bin/env python3`).
///
/// The input should be the first line of the content, already confirmed
/// to start with `#!`.
pub fn detect_by_shebang(first_line: &str) -> Option<&'static str> {
    for (regex, language) in SHEBANG_MAP.iter() {
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
