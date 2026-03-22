use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "java",
        extensions: &[".java"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"(?m)\bpublic\s+class\s+\w+", 4),
            wp!(r"\bpublic\s+static\s+void\s+main", 5),
            wp!(r"\bSystem\.out\.print(ln)?\s*\(", 5),
            wp!(r"(?m)\bimport\s+java\.\w+", 5),
            wp!(r"(?m)\bimport\s+javax\.\w+", 5),
            wp!(r"(?m)\bimport\s+org\.\w+", 4),
            wp!(r"@Override\b", 3),
            wp!(r"(?m)\bthrows\s+\w+", 2),
            wp!(r"\bextends\s+\w+", 1),
            wp!(r"\bimplements\s+\w+", 2),
            wp!(r"(?m)\bprivate\s+(final\s+)?\w+\s+\w+", 2),
        ],
        anti_patterns: &[
            wp!(r"=>\s*[\{(\n]", -3),
            wp!(r"\bfun\s+\w+", -4),
            wp!(r"\bval\s+\w+", -3),
            wp!(r"\bvar\s+\w+\s*[=:]", -2),
            wp!(r"\bcompanion\s+object\b", -5),
            wp!(r"\bdata\s+class\b", -5),
            wp!(r"\bsealed\s+class\b", -4),
            wp!(r"\bobject\s+\w+\s*:", -3),
        ],
        uses_hash_comments: false,
        keywords: &[
            "synchronized", "strictfp", "transient", "volatile", "native",
            "extends", "implements", "throws", "package", "final",
            "abstract", "static", "private", "protected", "public",
        ],
        builtins: &[
            "system", "override", "deprecated", "suppresswarnings",
            "arraylist", "hashmap", "hashset", "linkedlist", "treemap",
            "iterator", "comparable", "runnable", "serializable",
            "inputstream", "outputstream", "bufferedreader",
        ],
        family: Some("jvm-family"),
        exclusive_patterns: &[
            wp!(r"\bpublic\s+static\s+void\s+main\s*\(", 5),
            wp!(r"(?m)^\s*@Override\b", 3),
            wp!(r"(?m)^\s*import\s+java\.\w+", 4),
        ],
    }
}