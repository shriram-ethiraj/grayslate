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
        // ── Family-gated fields ───────────────────────────────
        content_families: &[ContentFamily::Code, ContentFamily::Config, ContentFamily::StructuredData],
        anchors: &[
            // Selector + declaration block
            wp!(r"(?m)[.#][\w\-]+\s*\{", 4),
            // @media query
            wp!(r"(?m)@media\s*[\s(]", 4),
            // @import url(
            wp!(r"@import\s+url\s*\(", 4),
            // :root {
            wp!(r":root\s*\{", 4),
            // @keyframes animation
            wp!(r"@keyframes\s+\w+", 5),
            // !important declaration
            wp!(r"!important\s*;", 4),
            // Tailwind / @apply directives
            wp!(r"@(tailwind|apply)\s", 4),
            // Element selector with block: body {, html {, div {, * {
            wp!(r"(?m)^\s*(body|html|div|main|section|article|header|footer|nav|aside)\s*\{", 4),
            // Property: value; pattern (CSS-specific syntax)
            wp!(r"(?m)^\s*[\w-]+\s*:\s*[^;]+;\s*$", 4),
        ],
        hints: &[
            wp!(r"\bbackground\s*:", 2),
            wp!(r"\bcolor\s*:", 2),
            wp!(r"\bmargin\s*:", 2),
            wp!(r"\bpadding\s*:", 2),
            wp!(r"\bdisplay\s*:", 2),
            // Pseudo-classes
            wp!(r":(hover|focus|active|first-child)\b", 2),
            // CSS custom properties
            wp!(r"var\(--", 3),
            // Flexbox / Grid display
            wp!(r"(display:\s*flex|display:\s*grid)", 2),
            // @font-face block
            wp!(r"@font-face\s*\{", 3),
            // Width/height
            wp!(r"\b(width|height)\s*:", 2),
            // Position/z-index
            wp!(r"\b(position|z-index)\s*:", 2),
            // CSS units
            wp!(r"\d+(px|em|rem|vh|vw|%)\b", 2),
        ],
        disqualifiers: &[
            // If content has `fun` keyword + `import` from packages, it's code not CSS
            wp!(r"(?m)^\s*(override\s+|internal\s+|private\s+)*fun\s+\w+", -5),
            // Java access modifiers
            wp!(r"(public|private|protected)\s+(class|void|static)", -5),
            // Python method with self
            wp!(r"def\s+\w+\(self", -5),
            // Clojure defn
            wp!(r"\(defn\s+", -5),
        ],
    }
}
