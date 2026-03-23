use super::{wp, LanguageDefinition};
use super::ContentFamily;

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
            // Broad fun match: `fun name(`, `fun name<`, `fun Type.name(`
            wp!(r"(?m)^\s*(override\s+|internal\s+|private\s+|public\s+|protected\s+|inline\s+|suspend\s+)*fun\s+\w+", 5),
            wp!(r"(?m)^\s*val\s+\w+\s*[=:]", 2),
            wp!(r"(?m)^\s*var\s+\w+\s*[=:]", 1),
            wp!(r"(?m)^\s*import\s+\w+\.\w+", 1),
            wp!(r"(?m)^\s*package\s+\w+\.\w+", 2),
            wp!(r"\bcompanion\s+object\b", 5),
            wp!(r"\bdata\s+class\s+\w+", 5),
            wp!(r"\bsealed\s+(class|interface)\s+\w+", 5),
            wp!(r"(?m)\bobject\s+\w+\s*[:\{]", 3),
            wp!(r"\bwhen\s*\(\w+\)\s*\{", 3),
            wp!(r"\b(listOf|mapOf|setOf|mutableListOf|arrayOf|mutableMapOf)\s*\(", 4),
            wp!(r"\bprintln\s*\(", 2),
            // Coroutines
            wp!(r"(?m)\b(suspend|coroutineScope)\s", 4),
            wp!(r"\blaunch\s*[\{(]", 4),
            wp!(r"\basync\s*\{", 4),
            wp!(r"\b(String|Int|Double|Boolean|Long|Float)\b", 1),
            // Kotlin-only syntax
            wp!(r"\btypealias\s+\w+", 5),
            wp!(r"@file:\w+", 5),
            wp!(r"\bvalue\s+class\s+\w+", 5),
            wp!(r"\bby\s+lazy\s*\{", 4),
            // Gradle Kotlin DSL patterns (common in .kts)
            wp!(r"(?m)^\s*plugins\s*\{", 3),
            wp!(r"(?m)^\s*dependencies\s*\{", 3),
            wp!(r"\bimplementation\s*\(", 3),
            wp!(r"\btestImplementation\s*\(", 3),
        ],
        anti_patterns: &[
            wp!(r"\bstd::\w+", -5),
            wp!(r"\bpublic\s+static\s+void\s+main\b", -5),
            wp!(r#"#include\s*[<"]"#, -5),
            // Not CSS
            wp!(r"(?m)^[.#][\w\-]+\s*\{[^}]*(color|margin|padding|display|font-size)\s*:", -5),
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
        exclusive_patterns: &[
            wp!(r"(?m)^\s*(override\s+|internal\s+|private\s+)*fun\s+\w+", 4),
            wp!(r"\bcompanion\s+object\b", 4),
            wp!(r"\bdata\s+class\s+\w+", 4),
        ],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            // Broad fun match — the defining Kotlin keyword
            wp!(r"(?m)^\s*(override\s+|internal\s+|private\s+|public\s+|protected\s+|inline\s+|suspend\s+)*fun\s+\w+", 5),
            wp!(r"\bcompanion\s+object\b", 5),
            wp!(r"\bdata\s+class\s+\w+", 5),
            wp!(r"\bsealed\s+(class|interface)\s+\w+", 5),
            wp!(r"\btypealias\s+\w+", 5),
            wp!(r"@file:\w+", 5),
            wp!(r"\bvalue\s+class\s+\w+", 5),
            wp!(r"\b(listOf|mapOf|setOf|mutableListOf|arrayOf|mutableMapOf)\s*\(", 4),
            wp!(r"(?m)\b(suspend|coroutineScope)\s", 4),
            // Kotlin extension function: `fun Type.name(`
            wp!(r"(?m)^\s*(private\s+|internal\s+)*fun\s+\w+\.\w+\s*\(", 5),
            // Gradle DSL (boosts .kts content)
            wp!(r"\bimplementation\s*\(", 4),
        ],
        hints: &[
            wp!(r"\bprintln\s*\(", 2),
            wp!(r"(?m)^\s*val\s+\w+\s*[=:]", 2),
            wp!(r"(?m)^\s*var\s+\w+\s*[=:]", 1),
            wp!(r"\?\.", 2),
            wp!(r"\?:", 2),
            wp!(r"\bby\s+lazy\s*\{", 3),
            // Scope functions with lambda
            wp!(r"\.\s*(let|also|apply|run)\s*\{", 3),
            // Gradle DSL
            wp!(r"(?m)^\s*plugins\s*\{", 2),
            wp!(r"(?m)^\s*dependencies\s*\{", 2),
            wp!(r"\bwhen\s*\(", 2),
        ],
        rivals: &["java", "scala"],
        differentiators: &[
            wp!(r"(?m)^\s*(override\s+|internal\s+|private\s+|public\s+|protected\s+|inline\s+|suspend\s+)*fun\s+\w+", 5),
            wp!(r"\bcompanion\s+object\b", 5),
            wp!(r"\bdata\s+class\s+\w+", 5),
            wp!(r"\btypealias\s+\w+", 5),
            wp!(r"@file:\w+", 5),
            wp!(r"(?m)^\s*val\s+\w+\s*[=:]", 3),
            wp!(r"(?m)^\s*var\s+\w+\s*[=:]", 2),
            wp!(r"\?\.", 3),
            wp!(r"\?:", 3),
            wp!(r"\bby\s+lazy\s*\{", 3),
            wp!(r"\.\s*(let|also|apply|run)\s*\{", 3),
        ],
        disqualifiers: &[
            wp!(r#"#include\s*[<"]"#, -5),
        ],
    }
}
