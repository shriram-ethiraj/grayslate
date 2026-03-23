use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "c",
        extensions: &[".c", ".h"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r#"(?m)#include\s*[<"]"#, 3),
            wp!(r"(?m)\bint\s+main\s*\(", 4),
            wp!(r"\bprintf\s*\(", 3),
            wp!(r"\b(malloc|calloc|realloc|free)\s*\(", 4),
            wp!(r"(?m)#define\s+\w+", 2),
            wp!(r"\btypedef\s+", 2),
            wp!(r"(?m)\bstruct\s+\w+\s*\{", 2),
            wp!(r"\bsizeof\s*\(", 2),
            wp!(r"\bNULL\b", 2),
            wp!(r"->\w+", 1),
            wp!(r"(?m)\bvoid\s+\w+\s*\(", 1),
            wp!(r"(?m)#(?:ifndef|ifdef)\s+\w+", 3),
            wp!(r"(?m)#pragma\s", 2),
        ],
        anti_patterns: &[
            wp!(r"\bstd::\w+", -5),
        ],
        uses_hash_comments: false,
        keywords: &[
            "auto", "register", "restrict", "sizeof", "typedef",
            "union", "volatile", "extern", "inline", "struct",
            "enum", "unsigned", "signed", "static", "const",
        ],
        builtins: &[
            "malloc", "calloc", "realloc", "free", "printf", "fprintf",
            "sprintf", "scanf", "sscanf", "memcpy", "memset", "memmove",
            "strlen", "strcpy", "strcat", "strcmp", "strncmp", "strtol",
            "fopen", "fclose", "fread", "fwrite", "fgets", "fputs",
        ],
        family: Some("c-family"),
        exclusive_patterns: &[
            wp!(r#"(?m)#include\s*[<"]"#, 3),
            wp!(r"(?m)#(?:ifndef|ifdef|define)\s+\w+", 3),
            wp!(r"(?m)#pragma\s", 2),
        ],
        // ── New family-gated fields ──────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r#"(?m)#include\s*[<"]"#, 4),
            wp!(r"(?m)#define\s+\w+", 4),
            wp!(r"(?m)#(?:ifndef|ifdef)\s+\w+", 4),
            wp!(r"(?m)\bint\s+main\s*\(", 4),
            wp!(r"\bprintf\s*\(", 4),
            wp!(r"\b(malloc|calloc|realloc|free)\s*\(", 4),
        ],
        hints: &[
            wp!(r"(?m)\bvoid\s+\w+\s*\(", 2),
            wp!(r"\bsizeof\s*\(", 2),
            wp!(r"\btypedef\s+", 2),
            wp!(r"(?m)\bstruct\s+\w+\s*\{", 2),
            wp!(r"\bNULL\b", 2),
        ],
        rivals: &["cpp"],
        differentiators: &[
            wp!(r"\bprintf\s*\(", 4),
            wp!(r"\b(malloc|calloc|realloc|free)\s*\(", 4),
            wp!(r"\btypedef\s+", 3),
            wp!(r"\bNULL\b", 2),
        ],
        disqualifiers: &[],
    }
}
