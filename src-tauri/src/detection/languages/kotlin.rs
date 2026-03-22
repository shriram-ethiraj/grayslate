use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "kotlin",
        extensions: &[".kt", ".kts"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"(?m)^\s*fun\s+\w+\s*[<(]", 5),
            wp!(r"(?m)^\s*val\s+\w+\s*[=:]", 2),
            wp!(r"(?m)^\s*var\s+\w+\s*[=:]", 1),
            wp!(r"(?m)^\s*import\s+\w+\.\w+", 1),
            wp!(r"(?m)^\s*package\s+\w+\.\w+", 2),
            wp!(r"\bcompanion\s+object\b", 5),
            wp!(r"\bdata\s+class\s+\w+", 5),
            wp!(r"\bsealed\s+class\s+\w+", 5),
            wp!(r"(?m)\bobject\s+\w+\s*[:\{]", 3),
            wp!(r"\bwhen\s*\(\w+\)\s*\{", 3),
            wp!(r"\b(listOf|mapOf|setOf|mutableListOf)\s*\(", 4),
            wp!(r"\bprintln\s*\(", 2),
            // Split coroutine keywords — "launch" and "async" are common English words
            wp!(r"(?m)\b(suspend|coroutineScope)\s", 4),
            wp!(r"\blaunch\s*[\{(]", 4),
            wp!(r"\basync\s*\{", 4),
            wp!(r"\b(String|Int|Double|Boolean|Long|Float)\b", 1),
        ],
        anti_patterns: &[
            wp!(r"\bstd::\w+", -5),
            wp!(r"\bpublic\s+static\s+void\s+main\b", -5),
        ],
        uses_hash_comments: false,
        keywords: &[
            "fun", "val", "var", "companion", "object", "data",
            "sealed", "lateinit", "suspend", "inline", "noinline",
            "crossinline", "reified", "tailrec", "operator", "infix",
            "typealias", "when", "init", "internal", "expect", "actual",
            "annotation", "inner", "out", "vararg", "by",
        ],
        builtins: &[
            "println", "listof", "mapof", "setof", "arrayof",
            "mutablelistof", "mutablemapof", "mutablesetof",
            "hashsetof", "hashmapof", "linkedmapof", "sortedsetof",
            "emptylist", "emptymap", "emptyset", "sequenceof",
            "todo", "check", "require", "repeat", "run", "apply",
            "also", "let", "with", "takeif", "takeunless",
        ],
        family: Some("jvm-family"),
        exclusive_patterns: &[],
    }
}
