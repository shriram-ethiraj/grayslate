use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "go",
        extensions: &[".go"],
        filenames: &["go.mod", "go.sum"],
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
            // Go type definitions
            wp!(r"(?m)^\s*type\s+\w+\s+struct\s*\{", 5),
            wp!(r"(?m)^\s*type\s+\w+\s+interface\s*\{", 4),
            // Go const block
            wp!(r"(?m)^\s*const\s+\(", 3),
            wp!(r"(?m)^package\s+\w+\s*\n\n\s*(import|func|type|const|var)\b", 5),
            // Go module patterns
            wp!(r"(?m)^module\s+\w+\.\w+[/\w]*", 5),
            wp!(r"(?m)^go\s+\d+\.\d+", 4),
        ],
        anti_patterns: &[
            wp!(r"(?m)\bclass\s+\w+", -5),
            wp!(r"(?m)^\s*import\s+\w+\s*$", -2),
            // Not Ruby
            wp!(r"(?m)^\s*end\s*$", -3),
            wp!(r"\battr_(accessor|reader|writer)\s+:", -5),
            wp!(r"\bdo\s*\|", -3),
            // Not Kotlin/Java
            wp!(r"(?m)^\s*fun\s+\w+", -4),
            wp!(r"\bdata\s+class\b", -5),
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
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"(?m)^package\s+\w+\s*$", 5),
            wp!(r"(?m)^\s*func\s+\w+\s*\(", 4),
            wp!(r"(?m)^\s*func\s+\(\w+\s+\*?\w+\)\s+\w+", 5),
            wp!(r"\bgo\s+func\b", 4),
            wp!(r"\bif\s+err\s*!=\s*nil\b", 4),
            wp!(r"\bfmt\.\w+", 4),
            wp!(r"\bchan\s+\w+", 4),
            wp!(r"\bpackage\s+main\b", 4),
            wp!(r"(?m)^\s*type\s+\w+\s+struct\s*\{", 5),
            // Go module file: `module github.com/...`
            wp!(r"(?m)^module\s+\w+\.\w+[/\w]*", 5),
            wp!(r"(?m)^go\s+\d+\.\d+", 4),
        ],
        hints: &[
            wp!(r"(?m)\bimport\s+\(", 3),
            wp!(r"(?m)\bdefer\s+\w+", 3),
            wp!(r":=\s", 2),
            wp!(r"\bmake\s*\(", 2),
            // `const` at file level (even simple `const Version = ...`)
            wp!(r"(?m)^\s*const\s+\w+\s*=", 2),
            wp!(r"(?m)^\s*var\s+\w+\s", 1),
            wp!(r"(?m)^\s*type\s+\w+\s+interface\s*\{", 3),
        ],
        rivals: &["rust", "c"],
        differentiators: &[
            wp!(r"(?m)^package\s+\w+\s*$", 5),
            wp!(r":=\s", 3),
            wp!(r"\bgo\s+func\b", 4),
            wp!(r"(?m)\bdefer\s+\w+", 3),
            wp!(r"(?m)^\s*func\s+\w+\s*\(", 4),
            wp!(r"(?m)^\s*type\s+\w+\s+struct\s*\{", 5),
        ],
        disqualifiers: &[],
    }
}