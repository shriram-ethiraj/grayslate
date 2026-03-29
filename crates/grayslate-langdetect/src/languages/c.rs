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
        keywords: &[
            "auto", "register", "restrict", "sizeof", "typedef",
            "union", "volatile", "extern", "inline", "struct",
            "enum", "unsigned", "signed", "static", "const",
            "goto",
        ],
        builtins: &[
            "malloc", "calloc", "realloc", "free", "printf", "fprintf",
            "sprintf", "scanf", "sscanf", "memcpy", "memset", "memmove",
            "strlen", "strcpy", "strcat", "strcmp", "strncmp", "strtol",
            "fopen", "fclose", "fread", "fwrite", "fgets", "fputs",
            "atoi", "atof", "getchar", "putchar", "perror", "exit",
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
            // C standard library headers (strong C signal)
            wp!(r#"(?m)#include\s*<(stdio|stdlib|string|stdint|stdbool|stddef|errno|signal|assert)\.h>"#, 5),
            // typedef struct — idiomatic C, rare in C++
            wp!(r"\btypedef\s+struct\b", 4),
            // fprintf/sprintf/snprintf — C I/O
            wp!(r"\b(fprintf|sprintf|snprintf|fscanf)\s*\(", 4),
            // extern "C" — header compatibility, strong C signal
            wp!(r#"extern\s+"C"\s*\{"#, 4),
            // Forward declarations with struct keyword (C-style)
            wp!(r"(?m)\btypedef\s+enum\b", 4),
        ],
        hints: &[
            wp!(r"(?m)\bvoid\s+\w+\s*\(", 2),
            wp!(r"\bsizeof\s*\(", 2),
            wp!(r"\btypedef\s+", 2),
            wp!(r"(?m)\bstruct\s+\w+\s*\{", 3),
            wp!(r"\bNULL\b", 3),
            wp!(r"->\w+", 2),
            // void* generic pointer — C idiom
            wp!(r"\bvoid\s*\*", 2),
            // unsigned/signed type declarations
            wp!(r"\b(unsigned|signed)\s+(int|char|long|short)\b", 2),
            wp!(r"(?m)#pragma\s+once\b", 3),
            // __cplusplus guard — found in C headers for C++ compat
            wp!(r"__cplusplus", 2),
            // #endif — present in virtually all C/C++ headers
            wp!(r"(?m)^#endif\b", 2),
        ],
        disqualifiers: &[
            // C++ exclusive syntax means this is NOT C
            wp!(r"\bstd::\w+", -5),
            wp!(r"(?m)\btemplate\s*<", -5),
            wp!(r"(?m)\bnamespace\s+\w+", -5),
            wp!(r"\bclass\s+\w+\s*[:\{]", -4),
            wp!(r"\bcout\s*<<", -5),
            wp!(r"\bcin\s*>>", -5),
        ],
    }
}
