use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "scala",
        extensions: &[".scala", ".sc", ".sbt"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"(?m)^\s*def\s+\w+\s*[(\[:]", 3),
            wp!(r"(?m)^\s*val\s+\w+\s*[=:]", 2),
            wp!(r"(?m)^\s*var\s+\w+\s*[=:]", 1),
            wp!(r"(?m)^\s*object\s+\w+\s*(extends|\{)", 4),
            wp!(r"(?m)^\s*trait\s+\w+", 4),
            wp!(r"(?m)^\s*case\s+class\s+\w+", 5),
            wp!(r"(?m)^\s*sealed\s+trait\b", 5),
            wp!(r"(?m)^\s*import\s+\w+\.(\w+\.)*\{", 3),
            wp!(r"(?m)^\s*import\s+scala\.\w+", 4),
            wp!(r"(?m)^\s*package\s+\w+\.\w+", 2),
            wp!(r"\b(List|Map|Set|Option|Either|Future|Seq)\s*[<\[(.]", 3),
            wp!(r"\bmatch\s*\{", 3),
            wp!(r"(?m)=>\s*$", 1),
            wp!(r"\bprintln\s*\(", 1),
            wp!(r"(?m)\bimplicit\s+(val|def|class)\b", 5),
            wp!(r"\bfor\s*\{", 2),
            // Scala-specific: class extends, with trait, lazy val
            wp!(r"\bclass\s+\w+.*\bextends\s+\w+", 4),
            wp!(r"\bextends\s+\w+\s+with\s+\w+", 2),
            wp!(r"\blazy\s+val\s+\w+", 4),
            // SBT-specific patterns
            wp!(r"\bThisBuild\b", 4),
            wp!(r"\bDef\.settings\b", 4),
            wp!(r#"%%?\s*""#, 3),
        ],
        anti_patterns: &[
            // Kotlin signals — penalize
            wp!(r"(?m)^\s*(override\s+|internal\s+|private\s+)*fun\s+\w+", -5),
            wp!(r"\bcompanion\s+object\b", -5),
            wp!(r"\bdata\s+class\b", -5),
            wp!(r"\btypealias\s+\w+", -5),
            wp!(r"@file:\w+", -5),
            // Go signals
            wp!(r"(?m)^package\s+\w+\s*$", -3),
            wp!(r"\bfunc\s+\w+\s*\(", -4),
            wp!(r":=\s", -3),
        ],
        uses_hash_comments: false,
        keywords: &[
            "def", "val", "var", "object", "trait", "sealed",
            "implicit", "lazy", "override", "abstract", "final",
            "match", "yield", "forsome", "type", "with", "given",
            "using", "export", "opaque", "extension", "derives",
        ],
        builtins: &[
            "option", "some", "none", "either", "left", "right",
            "list", "seq", "vector", "map", "set", "tuple",
            "future", "promise", "try", "success", "failure",
            "stream", "lazylist", "range", "bigdecimal", "bigint",
            "ordering", "unit", "nothing", "any", "anyval", "anyref",
        ],
        family: Some("jvm-family"),
        exclusive_patterns: &[
            wp!(r"(?m)^\s*case\s+class\s+\w+", 4),
            wp!(r"(?m)^\s*sealed\s+trait\b", 4),
            wp!(r"(?m)\bimplicit\s+(val|def|class)\b", 4),
        ],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"(?m)^\s*case\s+class\s+\w+", 5),
            wp!(r"(?m)^\s*sealed\s+trait\b", 5),
            wp!(r"(?m)\bimplicit\s+(val|def|class)\b", 5),
            wp!(r"(?m)^\s*object\s+\w+\s*(extends|\{)", 4),
            // `class X extends Y` — Scala uses `extends`, Kotlin uses `:`
            wp!(r"\bclass\s+\w+.*\bextends\s+\w+", 4),
            wp!(r"(?m)^\s*trait\s+\w+", 4),
            wp!(r"(?m)^\s*import\s+scala\.\w+", 4),
            // SBT
            wp!(r"\bThisBuild\b", 4),
            wp!(r"\blazy\s+val\s+\w+", 4),
        ],
        hints: &[
            wp!(r"(?m)^\s*def\s+\w+\s*[(\[:]", 3),
            wp!(r"(?m)^\s*val\s+\w+\s*[=:]", 2),
            wp!(r"(?m)^\s*var\s+\w+\s*[=:]", 2),
            wp!(r"\bextends\s+\w+", 3),
            wp!(r"\boverride\s+def\b", 3),
            wp!(r"\bmatch\s*\{", 3),
            wp!(r"\bextends\s+\w+\s+with\s+\w+", 2),
            wp!(r"\bfor\s*\{", 2),
            // Scala type parameters use brackets: `Seq[Type]`, `Option[Type]`
            wp!(r"\b(Seq|Option|Either|Future|List|Map|Set|Vector)\s*\[", 3),
            wp!(r"\bclassOf\s*\[", 3),
            // `private[scope]` access modifier — Scala-only
            wp!(r"\bprivate\s*\[\w+\]", 3),
        ],
        rivals: &["java", "kotlin"],
        differentiators: &[
            wp!(r"(?m)^\s*case\s+class\s+\w+", 5),
            wp!(r"(?m)^\s*sealed\s+trait\b", 5),
            wp!(r"(?m)\bimplicit\s+(val|def|class)\b", 5),
            wp!(r"(?m)^\s*object\s+\w+\s*(extends|\{)", 4),
            wp!(r"\bmatch\s*\{", 3),
            wp!(r"(?m)^\s*def\s+\w+\s*[(\[:]", 3),
            wp!(r"\bclass\s+\w+.*\bextends\s+\w+", 4),
            wp!(r"(?m)^\s*trait\s+\w+", 4),
            wp!(r"\blazy\s+val\s+\w+", 4),
            wp!(r"\bextends\s+\w+\s+with\s+\w+", 2),
        ],
        disqualifiers: &[],
    }
}