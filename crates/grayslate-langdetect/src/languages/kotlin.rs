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
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code, ContentFamily::Config],
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
            // lateinit var — Kotlin-only
            wp!(r"\blateinit\s+var\s+", 5),
            // Kotlin standard library imports
            wp!(r"(?m)\bimport\s+kotlin\.\w+", 5),
            wp!(r"(?m)\bimport\s+kotlinx\.\w+", 5),
            // Kotlin JVM interop annotations
            wp!(r"@Jvm(Static|Overloads|Field)\b", 4),
            // package declaration — shared with Java but strong Code signal
            wp!(r"(?m)^\s*package\s+[\w.]+", 4),
            // object declaration — Kotlin singleton
            wp!(r"(?m)^\s*object\s+\w+", 4),
            // val/var with type annotation: val x: Type
            wp!(r"\b(val|var)\s+\w+\s*:\s*\w+", 4),
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
            // Kotlin initializer block
            wp!(r"(?m)^\s*init\s*\{", 2),
            // Kotlin reflection
            wp!(r"::class\b", 2),
            // @Composable — Jetpack Compose
            wp!(r"@Composable\b", 3),
            // import org.* or import com.* — JVM ecosystem
            wp!(r"(?m)\bimport\s+(org|com|io)\.\w+", 3),
            // String templates: "$variable" or "${expr}"
            wp!(r#"\$\{?\w+"#, 2),
        ],
        disqualifiers: &[
            wp!(r#"#include\s*[<"]"#, -5),
        ],
    }
}
