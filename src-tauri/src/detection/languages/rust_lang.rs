use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "rust",
        extensions: &[".rs"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"(?m)^\s*fn\s+\w+\s*[<(]", 3),
            wp!(r"(?m)^\s*pub\s+(fn|struct|enum|mod|trait|impl)\s", 4),
            wp!(r"(?m)^\s*let\s+mut\s+\w+", 4),
            wp!(r"(?m)^\s*impl\s+\w+", 4),
            wp!(r"(?m)^\s*use\s+\w+(::\w+)+", 3),
            wp!(r"(?m)^\s*match\s+\w+\s*\{", 3),
            wp!(r"\b(Vec|Option|Result|Box|Rc|Arc|String)<\w", 4),
            wp!(r"\bprintln!\s*\(", 5),
            wp!(r"\b\w+\.unwrap\(\)", 3),
            wp!(r"(?m)^\s*#\[derive\(", 5),
            wp!(r"(?m)^\s*mod\s+\w+\s*[;\{]", 2),
            wp!(r"&mut\s+\w+", 3),
            wp!(r#"(?m)^\s*extern\s+"C""#, 3),
            wp!(r"(?m)^\s*trait\s+\w+", 3),
            // Inner attributes: #![...]
            wp!(r"(?m)^\s*#!\[", 5),
            // macro_rules! definition
            wp!(r"(?m)^\s*macro_rules!\s+\w+", 5),
            // Lifetime annotations: 'a, 'static
            wp!(r"[&<]\s*'\w+", 3),
            // Raw strings: r#"..."#
            wp!(r#"r#""#, 3),
        ],
        anti_patterns: &[
            wp!(r"\bself\.\w+", -1),
            wp!(r"(?m)class\s+\w+", -5),
        ],
        uses_hash_comments: false,
        keywords: &[
            "fn", "let", "mut", "pub", "mod", "crate", "trait", "impl",
            "dyn", "ref", "unsafe", "extern", "async", "await",
            "match", "loop", "move", "where", "self",
        ],
        builtins: &[
            "option", "result", "vec", "string", "box", "rc", "arc",
            "cell", "refcell", "mutex", "rwlock", "hashmap", "hashset",
            "btreemap", "btreeset", "cow", "pin", "phantom",
            "iterator", "intoiterator", "from", "into", "display",
            "debug", "clone", "copy", "send", "sync", "sized",
            "drop", "default", "partialeq", "partialord",
        ],
        family: None,
        exclusive_patterns: &[
            wp!(r"(?m)^\s*fn\s+\w+\s*[<(]", 4),
            wp!(r"(?m)#\[derive\(", 4),
            wp!(r"\blet\s+mut\s+", 3),
            wp!(r"\bimpl\s+\w+", 3),
        ],
    }
}