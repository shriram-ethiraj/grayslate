use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "rust",
        extensions: &[".rs"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        keywords: &[
            "fn", "let", "mut", "pub", "mod", "use", "crate", "trait", "impl",
            "struct", "enum", "type", "const", "static", "super",
            "dyn", "ref", "unsafe", "extern", "async", "await",
            "match", "loop", "move", "where", "self",
        ],
        builtins: &[
            "option", "result", "some", "none", "ok", "err",
            "vec", "string", "box", "rc", "arc",
            "cell", "refcell", "mutex", "rwlock", "hashmap", "hashset",
            "btreemap", "btreeset", "cow", "pin", "phantom",
            "iterator", "intoiterator", "from", "into", "display",
            "debug", "clone", "copy", "send", "sync", "sized",
            "drop", "default", "deref", "asref", "future",
            "partialeq", "partialord", "tostring",
        ],
        // ── Family-gated fields ───────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"(?m)^\s*fn\s+\w+\s*[<(]", 4),
            // fn main() — Rust entry point
            wp!(r"(?m)^\s*fn\s+main\s*\(\s*\)", 5),
            wp!(r"(?m)^\s*#\[derive\(", 5),
            wp!(r"(?m)^\s*let\s+mut\s+\w+", 4),
            wp!(r"(?m)^\s*impl\s+\w+", 4),
            // impl Trait for Type — trait implementation
            wp!(r"(?m)^\s*impl\s+\w+\s+for\s+\w+", 5),
            wp!(r"(?m)^\s*pub\s+(fn|struct|enum|mod|trait|impl)\s", 4),
            wp!(r"(?m)^\s*macro_rules!\s+\w+", 5),
            wp!(r"\bprintln!\s*\(", 5),
            wp!(r"\beprintln!\s*\(", 5),
            wp!(r"(?m)^\s*#!\[", 5),
            // Result<T, E> and Option<T> — ubiquitous Rust types
            wp!(r"\bResult<\w", 4),
            wp!(r"\bOption<\w", 4),
            // match block — Rust pattern matching
            wp!(r"(?m)^\s*match\s+\w+\s*\{", 4),
            // Lifetime annotations — uniquely Rust
            wp!(r"<'[a-z]", 4),
            wp!(r"&'[a-z]\s", 4),
            // unsafe block
            wp!(r"\bunsafe\s*\{", 4),
            // mod tests — Rust testing convention
            wp!(r"(?m)^\s*mod\s+tests\s*\{", 4),
            // #[cfg(test)] — Rust test attribute
            wp!(r"(?m)^\s*#\[cfg\(test\)\]", 5),
        ],
        hints: &[
            wp!(r"(?m)^\s*use\s+\w+(::\w+)+", 3),
            wp!(r"\b\w+\.unwrap\(\)", 3),
            wp!(r"&mut\s+\w+", 3),
            wp!(r"(?m)^\s*mod\s+\w+\s*[;\{]", 2),
            // Fat arrow in match arms
            wp!(r"\s=>\s", 2),
            // vec![] — Rust vector macro
            wp!(r"\bvec!\s*\[", 3),
            // todo!() / unimplemented!() — Rust placeholder macros
            wp!(r"\btodo!\s*\(", 3),
            wp!(r"\bunimplemented!\s*\(", 3),
            // use std:: — Rust standard library import
            wp!(r"\buse\s+std::", 3),
            // pub(crate) — Rust visibility modifier
            wp!(r"\bpub\s*\(\s*crate\s*\)", 3),
            // #[cfg(...)] — Rust conditional compilation
            wp!(r"(?m)^\s*#\[cfg\(", 3),
            // #[test] — Rust test attribute
            wp!(r"(?m)^\s*#\[test\]", 3),
            // format!() — Rust formatting macro
            wp!(r"\bformat!\s*\(", 3),
            // assert!/assert_eq! — Rust assertion macros
            wp!(r"\bassert(_eq|_ne)!\s*\(", 2),
        ],
        disqualifiers: &[
            // C++ — std::cout output
            wp!(r"\bstd::cout\b", -5),
            // C/C++ — #include directive
            wp!(r"(?m)^\s*#include\s", -5),
            // C++ — template syntax
            wp!(r"\btemplate\s*<", -4),
            // Go — func keyword for functions
            wp!(r"(?m)^\s*func\s+\w+\s*\(", -4),
            // Go — package declaration
            wp!(r"(?m)^package\s+main\b", -5),
        ],
    }
}
