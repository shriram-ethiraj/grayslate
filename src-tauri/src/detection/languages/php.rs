use regex::Regex;
use std::sync::LazyLock;

use super::{wp, LanguageDefinition};

/// Structural detection for PHP content.
pub(crate) fn is_likely_php(trimmed: &str, _was_sliced: bool) -> bool {
    static PHP_OPEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^<\?php\b").unwrap());
    if PHP_OPEN.is_match(trimmed) {
        return true;
    }
    if trimmed.starts_with("<?") && !trimmed.starts_with("<?xml") {
        static PHP_VAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$\w+\s*=").unwrap());
        static PHP_ECHO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\becho\s").unwrap());
        if PHP_VAR.is_match(trimmed) || PHP_ECHO.is_match(trimmed) {
            return true;
        }
    }
    false
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "php",
        extensions: &[".php", ".php3", ".php4", ".php5", ".php7", ".phtml"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[r"\bphp\b"],
        structural_priority: Some(10),
        structural_detect: Some(is_likely_php),
        patterns: &[
            wp!(r"<\?php\b", 5),
            wp!(r"\$\w+\s*=\s*", 2),
            wp!(r"\$this->\w+", 4),
            wp!(r"(?m)\bfunction\s+\w+\s*\(", 2),
            wp!(r#"\becho\s+['"\$]"#, 3),
            wp!(r"(?m)\b(public|private|protected)\s+function\b", 4),
            wp!(r"(?m)\bnamespace\s+\w+(\\\w+)*", 3),
            wp!(r"(?m)\buse\s+\w+(\\\w+)+\s*;", 3),
            wp!(r"\bnew\s+\w+\s*\(", 1),
            wp!(r"->\w+\s*\(", 2),
            wp!(r"\b(array|isset|unset|empty|die|exit)\s*\(", 3),
            wp!(r"\$_?(GET|POST|REQUEST|SESSION|SERVER|COOKIE)\b", 5),
            wp!(r"(?m)\bclass\s+\w+\s*(extends|implements)\b", 2),
        ],
        anti_patterns: &[
            wp!(r"=>\s*[\{(\n]", -2),
        ],
        uses_hash_comments: false,
        keywords: &[
            "echo", "print", "die", "exit", "include", "include_once",
            "require", "require_once", "isset", "empty", "unset",
            "list", "array", "eval", "namespace", "use", "yield",
            "global", "static", "final", "abstract", "extends",
            "implements", "instanceof", "foreach", "elseif",
            "endif", "endfor", "endforeach", "endwhile", "endswitch",
            "enddeclare", "declare", "trait", "insteadof",
        ],
        builtins: &[
            "count", "strlen", "substr", "strpos", "str_replace",
            "array_push", "array_pop", "array_shift", "array_merge",
            "array_map", "array_filter", "array_keys", "array_values",
            "explode", "implode", "json_encode", "json_decode",
            "var_dump", "print_r", "preg_match", "preg_replace",
            "file_get_contents", "file_put_contents", "is_array",
        ],
        family: None,
        exclusive_patterns: &[],
    }
}
