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
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"(?m)^package\s+\w+\s*$", 5),
            wp!(r"(?m)^\s*func\s+\w+\s*\(", 4),
            // Method with receiver: func (s *Server) Start(
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
            // func init() — Go initialization
            wp!(r"(?m)^\s*func\s+init\s*\(\s*\)", 4),
            // make(map[...], make([]..., make(chan
            wp!(r"\bmake\s*\(\s*(map\[|chan\s|\[\])", 4),
            // Goroutine launch: go someFunc(
            wp!(r"\bgo\s+\w+\s*\(", 4),
            // Go type assertion: x.(Type)
            wp!(r"\.\(\w+\)", 3),
        ],
        hints: &[
            wp!(r"(?m)\bimport\s+\(", 3),
            wp!(r"(?m)\bdefer\s+\w+", 3),
            wp!(r":=\s", 3),
            wp!(r"\bmake\s*\(", 2),
            // `const` at file level (even simple `const Version = ...`)
            wp!(r"(?m)^\s*const\s+\w+\s*=", 2),
            wp!(r"(?m)^\s*var\s+\w+\s", 1),
            wp!(r"(?m)^\s*type\s+\w+\s+interface\s*\{", 3),
            // for ... range — Go iteration
            wp!(r"\brange\s+\w+", 2),
            // Go error return pattern: return nil, err
            wp!(r"\breturn\s+\w+,\s*err\b", 3),
            // context.Context — ubiquitous Go type
            wp!(r"\bcontext\.Context\b", 3),
            // interface{} / any — Go empty interface
            wp!(r"\binterface\{\}", 2),
        ],
        disqualifiers: &[
            // Rust macros — strong Rust signal
            wp!(r"\bprintln!\s*\(", -5),
            wp!(r"(?m)^\s*#\[derive\(", -5),
        ],
    }
}
