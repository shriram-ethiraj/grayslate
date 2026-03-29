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
        // ── Family-gated fields ───────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"(?m)^\s*case\s+class\s+\w+", 5),
            wp!(r"(?m)^\s*sealed\s+trait\b", 5),
            wp!(r"(?m)\bimplicit\s+(val|def|class)\b", 5),
            wp!(r"(?m)^\s*object\s+\w+\s*(extends|\{)", 4),
            // `class X extends Y` â€” Scala uses `extends`, Kotlin uses `:`
            wp!(r"\bclass\s+\w+.*\bextends\s+\w+", 4),
            wp!(r"(?m)^\s*trait\s+\w+", 4),
            wp!(r"(?m)^\s*import\s+scala\.\w+", 4),
            // SBT
            wp!(r"\bThisBuild\b", 4),
            wp!(r"\blazy\s+val\s+\w+", 4),
            // `extends App` — Scala's App trait
            wp!(r"\bextends\s+App\b", 5),
            // Scala 3 `given` instances
            wp!(r"(?m)^\s*given\s+\w+", 4),
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
            // `private[scope]` access modifier â€” Scala-only
            wp!(r"\bprivate\s*\[\w+\]", 3),
            // Pattern match arms: `case x =>`
            wp!(r"\bcase\s+\w+\s*=>", 3),
            // Scala type bounds: `<:` upper, `>:` lower
            wp!(r"[<>]:\s*\w+", 2),
            // Type alias: `type Foo =`
            wp!(r"(?m)^\s*type\s+\w+\s*=", 2),
            // For-comprehension yield
            wp!(r"\byield\s+", 2),
        ],
        disqualifiers: &[
            // Java signals
            wp!(r"\bpublic\s+static\s+void\s+main\b", -5),
            wp!(r"@Override\b", -4),
            // Kotlin signals
            wp!(r"\bfun\s+\w+", -5),
            wp!(r"\bdata\s+class\s+\w+", -5),
        ],
    }
}