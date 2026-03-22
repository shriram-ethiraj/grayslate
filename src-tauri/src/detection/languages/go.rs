use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "go",
        extensions: &[".go"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"(?m)^package\s+\w+\s*$", 5),
            wp!(r"(?m)^\s*func\s+\w+\s*\(", 3),
            wp!(r"(?m)^\s*func\s+\(\w+\s+\*?\w+\)\s+\w+", 5),
            wp!(r"\bfmt\.\w+", 4),
            wp!(r"(?m)\bimport\s+\(", 3),
            wp!(r"\bgo\s+func\b", 4),
            wp!(r"\bchan\s+\w+", 4),
            wp!(r":=\s", 2),
            wp!(r"\bif\s+err\s*!=\s*nil\b", 4),
            wp!(r"(?m)\bdefer\s+\w+", 3),
            wp!(r"\bpackage\s+main\b", 4),
        ],
        anti_patterns: &[
            wp!(r"(?m)\bclass\s+\w+", -5),
            wp!(r"(?m)^\s*import\s+\w+\s*$", -2),
        ],
        uses_hash_comments: false,
        keywords: &[
            "func", "package", "chan", "defer", "go", "select",
            "range", "fallthrough", "goto", "struct", "interface",
            "map", "type", "const", "var",
        ],
        builtins: &[
            "append", "cap", "close", "complex", "copy", "delete",
            "imag", "len", "make", "panic", "println", "print",
            "real", "recover", "goroutine",
        ],
        family: None,
        exclusive_patterns: &[
            wp!(r"(?m)^package\s+\w+", 3),
            wp!(r"(?m)^\s*func\s+\w+\s*\(", 4),
            wp!(r":=\s", 3),
            wp!(r"\bgo\s+func\s*\(", 4),
        ],
    }
}