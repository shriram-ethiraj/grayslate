use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "css",
        extensions: &[".css", ".less"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"(?m)[.#][\w\-]+\s*\{", 3),
            wp!(r"(?m)@media\s*[\s(]", 4),
            wp!(r"@keyframes\s+\w+", 4),
            wp!(r"@import\s+", 2),
            wp!(r"!important\s*;", 3),
            wp!(r":hover|:focus|:active|::before|::after", 3),
            wp!(r"\bvar\s*\(--[\w\-]+\)", 3),
            wp!(r"(?m)\b(color|margin|padding|display|font-size|background|border|width|height)\s*:", 2),
            wp!(r"\b(flex|grid|block|inline|none)\s*;", 1),
            wp!(r"@tailwind|@apply", 3),
        ],
        anti_patterns: &[
            wp!(r"(?m)^\s*(function|const|let|var)\s", -5),
            wp!(r"\bpublic\s+class\s+\w+", -5),
            wp!(r"(?m)\bimport\s+java\.\w+", -5),
            wp!(r"\bpublic\s+static\s+void\s+main", -5),
            wp!(r"\bSystem\.out\.print", -3),
        ],
        uses_hash_comments: false,
        keywords: &[
            "important", "media", "keyframes", "supports", "font-face",
            "charset", "namespace", "viewport", "counter-style", "property",
            "layer", "container",
        ],
        builtins: &[
            "hover", "focus", "active", "visited", "before", "after",
            "first-child", "last-child", "nth-child", "placeholder",
            "selection", "root",
        ],
        illegal: None,
        extends: None,
    }
}
