use regex::Regex;
use std::sync::LazyLock;

use super::{wp, LanguageDefinition};
use super::ContentFamily;

/// Structuraldetection for PHP content.
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
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"<\?php\b", 5),
            wp!(r"\$_?(GET|POST|REQUEST|SESSION|SERVER|COOKIE)\b", 5),
            wp!(r"\$this->\w+", 4),
            wp!(r"(?m)\b(public|private|protected)\s+function\b", 4),
            // namespace App\Http\Controllers; — PHP namespace with backslashes
            wp!(r"(?m)^\s*namespace\s+\w+(\\\w+)+\s*;", 5),
            // use App\Models\User; — PHP use with backslash
            wp!(r"(?m)^\s*use\s+\w+(\\\w+)+\s*;", 4),
            // fn() => — PHP arrow functions (PHP 7.4+)
            wp!(r"\bfn\s*\([^)]*\)\s*=>", 4),
            // ::class — PHP class constant reference
            wp!(r"\w+::class\b", 3),
        ],
        hints: &[
            wp!(r"(?m)\bnamespace\s+\w+(\\\w+)*", 3),
            wp!(r"\becho\s+['\x22\$]", 3),
            wp!(r"\b(array|isset|unset|empty|die|exit)\s*\(", 3),
            // ?-> null-safe operator (PHP 8.0+)
            wp!(r"\?->\w+", 3),
            // match() expression (PHP 8.0+)
            wp!(r"\bmatch\s*\(", 2),
            // PHP enum (PHP 8.1+)
            wp!(r"(?m)^\s*enum\s+\w+\s*[:\{]", 3),
            // #[Attribute] — PHP 8.0 attributes
            wp!(r"(?m)^\s*#\[\w+", 2),
            // $variable = — PHP variable assignment
            wp!(r"\$\w+\s*=\s*", 2),
            // -> chained method call
            wp!(r"->\w+\s*\(", 2),
        ],
        disqualifiers: &[],
    }
}
