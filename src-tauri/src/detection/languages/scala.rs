use super::{wp, LanguageDefinition};

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
            wp!(r"(?m)^\s*def\s+\w+\s*[(\[]", 2),
            wp!(r"(?m)^\s*val\s+\w+\s*[=:]", 2),
            wp!(r"(?m)^\s*var\s+\w+\s*[=:]", 1),
            wp!(r"(?m)^\s*object\s+\w+\s*(extends|\{)", 4),
            wp!(r"(?m)^\s*trait\s+\w+", 3),
            wp!(r"(?m)^\s*case\s+class\s+\w+", 5),
            wp!(r"(?m)^\s*sealed\s+trait\b", 5),
            wp!(r"(?m)^\s*import\s+\w+\.(\w+\.)*\{", 3),
            wp!(r"(?m)^\s*package\s+\w+\.\w+", 2),
            // Require generic/call syntax — bare "Set", "Map", "List" are too common in English
            wp!(r"\b(List|Map|Set|Option|Either|Future|Seq)\s*[<\[(.]", 3),
            wp!(r"\bmatch\s*\{", 2),
            wp!(r"(?m)=>\s*$", 1),
            wp!(r"\bprintln\s*\(", 1),
            wp!(r"(?m)\bimplicit\s+(val|def|class)\b", 5),
            wp!(r"\bfor\s*\{", 2),
        ],
        anti_patterns: &[],
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
        exclusive_patterns: &[],
    }
}
