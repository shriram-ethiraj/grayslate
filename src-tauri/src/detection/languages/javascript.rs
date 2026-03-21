use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "javascript",
        extensions: &[".js", ".mjs", ".cjs", ".jsx"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[r"\bnode(js)?\b"],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"(?m)\b(const|let|var)\s+\w+\s*=", 2),
            wp!(r"(?m)\bfunction\s+\w*\s*\(", 2),
            wp!(r"=>\s*[\{(\n]", 3),
            wp!(r#"\brequire\s*\(['"`]"#, 4),
            wp!(r"\bmodule\.exports\b", 4),
            wp!(r"\bconsole\.\w+\s*\(", 2),
            wp!(r"===|!==", 2),
            wp!(r"\bdocument\.\w+", 2),
            wp!(r"\bwindow\.\w+", 1),
            wp!(r"\bPromise\.(all|resolve|reject)\b", 2),
            wp!(r"\.then\s*\(", 1),
            wp!(r"\.catch\s*\(", 1),
            wp!(r"(?m)\basync\s+(function|\w+\s*=>|\w+\s*\()", 2),
            wp!(r"\bawait\s+", 1),
            wp!(r#"(?m)^\s*import\s+[\w\{*].*\s+from\s+['"`]"#, 3),
            wp!(r"(?m)^\s*export\s+(const|let|var|function|class|default)\s", 3),
        ],
        anti_patterns: &[
            // TypeScript-only syntax in JS context
            wp!(r":\s*(string|number|boolean|void)\b", -3),
            wp!(r"(?m)\binterface\s+\w+\s*\{", -4),
            wp!(r"(?m)\btype\s+\w+\s*=\s*", -4),
            wp!(r"\b(keyof|typeof\s+\w+\s*===)\b", -2),
        ],
        uses_hash_comments: false,
        keywords: &[
            "const", "let", "var", "function", "typeof", "instanceof", "undefined",
            "void", "delete", "yield", "async", "await", "of",
        ],
        builtins: &[
            "console", "require", "exports", "module", "promise",
            "arraybuffer", "dataview", "weakmap", "weakset", "weakref",
            "proxy", "reflect", "symbol", "bigint", "nan", "infinity",
            "globalthis", "settimeout", "setinterval", "fetch",
        ],
        illegal: None,
        extends: None,
    }
}
