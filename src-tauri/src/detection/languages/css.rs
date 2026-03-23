use super::{wp, LanguageDefinition};
use super::ContentFamily;

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
            // Not Kotlin — `fun`, `val`, `import pkg.Class`, `package pkg.sub`
            wp!(r"(?m)^\s*(override\s+|internal\s+|private\s+)*fun\s+\w+", -5),
            wp!(r"\bcompanion\s+object\b", -5),
            wp!(r"\bdata\s+class\s+\w+", -5),
            wp!(r"\btypealias\s+\w+", -5),
            wp!(r"@file:\w+", -5),
            wp!(r"(?m)^\s*import\s+\w+\.\w+\.\w+", -4),
            wp!(r"(?m)^\s*package\s+\w+\.\w+", -4),
            // Not Clojure — S-expression patterns
            wp!(r"\(defn\s+\w+", -5),
            wp!(r"\(defproject\s+\w+", -5),
            wp!(r"(?m)^\s*\(ns\s+[\w.\-]+", -5),
            // Not Scala
            wp!(r"(?m)^\s*case\s+class\s+\w+", -5),
            wp!(r"(?m)^\s*sealed\s+trait\b", -5),
            wp!(r"(?m)^\s*def\s+\w+\s*[(\[]", -4),
            // Gradle DSL — looks like CSS selectors but isn't
            wp!(r"\bimplementation\s*\(", -4),
            wp!(r"\btestImplementation\s*\(", -4),
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
        family: None,
        exclusive_patterns: &[],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            // Selector + declaration block
            wp!(r"(?m)[.#][\w\-]+\s*\{", 4),
            // @media query
            wp!(r"(?m)@media\s*[\s(]", 4),
            // @import url(
            wp!(r"@import\s+url\s*\(", 4),
            // :root {
            wp!(r":root\s*\{", 4),
        ],
        hints: &[
            wp!(r"\bbackground\s*:", 2),
            wp!(r"\bcolor\s*:", 2),
            wp!(r"\bmargin\s*:", 2),
            wp!(r"\bpadding\s*:", 2),
            wp!(r"\bdisplay\s*:", 2),
        ],
        rivals: &["scss", "sass"],
        differentiators: &[
            wp!(r"\bvar\s*\(--[\w\-]+\)", 3),
            wp!(r"@import\s+url\s*\(", 3),
        ],
        disqualifiers: &[
            // If content has `fun` keyword + `import` from packages, it's code not CSS
            wp!(r"(?m)^\s*(override\s+|internal\s+|private\s+)*fun\s+\w+", -5),
        ],
    }
}
