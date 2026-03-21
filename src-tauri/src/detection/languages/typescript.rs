use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "typescript",
        extensions: &[".ts", ".tsx", ".mts", ".cts"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[r"\bdeno\b"],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"(?m)\binterface\s+\w+", 4),
            wp!(r"(?m)\btype\s+\w+\s*=\s*", 4),
            wp!(r":\s*(string|number|boolean|void|any|never|unknown|undefined)\b", 3),
            wp!(r"(?m)\benum\s+\w+\s*\{", 4),
            wp!(r"(?m)\bnamespace\s+\w+", 3),
            wp!(r"(?m)\bdeclare\s+(const|function|class|module|type|interface)", 4),
            wp!(r"\b(Readonly|Partial|Record|Pick|Omit|Required)<", 4),
            wp!(r"\bas\s+(string|number|any|unknown|[A-Z]\w+)\b", 3),
            wp!(r"(?m)^///\s*<reference\s", 5),
            wp!(r#"(?m)^\s*import\s+[\w\{*].*\s+from\s+['"`]"#, 2),
            wp!(r"(?m)^\s*export\s+(const|let|var|function|class|default|type|interface|enum)\s", 2),
            wp!(r"<\w+(\s+extends\s+\w+)?>", 2),
            wp!(r"(?m)\b(const|let|var)\s+\w+\s*=", 1),
            wp!(r"=>\s*[\{(\n]", 1),
            wp!(r"===|!==", 1),
            wp!(r"\b(keyof|infer|satisfies)\s+", 5),
            wp!(r"\w+\?\s*:\s*(string|number|boolean|any|\w+)", 3),
            wp!(r"\bas\s+const\b", 4),
            wp!(r"\b(Exclude|Extract|NonNullable|ReturnType|InstanceType|Parameters)<", 4),
            wp!(r"\|\s*(string|number|boolean|null|undefined)\b", 3),
            wp!(r":\s*\w+\[\]", 2),
            wp!(r"(?m)\bfunction\s+\w+\s*<\w+", 3),
            wp!(r"\bReact\.\w+<", 2),
        ],
        anti_patterns: &[],
        uses_hash_comments: false,
        keywords: &[
            "interface", "type", "namespace", "declare", "abstract",
            "readonly", "enum", "override", "satisfies", "keyof", "infer",
            "implements", "private", "protected", "public",
        ],
        builtins: &[
            "readonly", "partial", "record", "pick", "omit", "required",
            "exclude", "extract", "nonnullable", "returntype", "instancetype",
            "parameters", "awaited", "uppercase", "lowercase", "capitalize",
        ],
        illegal: None,
        // extends: Some("javascript"),  // disabled: causes TS to outscore JS on pure JS content
        extends: None,
    }
}
