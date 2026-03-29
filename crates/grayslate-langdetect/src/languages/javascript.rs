use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "javascript",
        extensions: &[".js", ".mjs", ".cjs", ".jsx"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[r"\bnode(js)?\b"],
        structural_priority: None,
        structural_detect: None,
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
        // ── New family-gated fields ──────────────────────────
        content_families: &[ContentFamily::Code, ContentFamily::Config, ContentFamily::StructuredData],
        anchors: &[
            wp!(r#"\brequire\s*\(['"`]"#, 4),
            wp!(r"\bmodule\.exports\b", 4),
            wp!(r"\bconsole\.\w+\s*\(", 4),
            wp!(r"(?m)\b(const|let)\s+\w+\s*=\s*require\s*\(", 5),
            wp!(r"\bexports\.\w+\s*=", 4),
            wp!(r"(?m)^\s*export\s+(const|let|var|function|class|default)\s", 4),
            wp!(r#"(?m)^\s*import\s+[\w\{*].*\s+from\s+['"`]"#, 4),
            // Arrow functions with body — very common JS/TS
            wp!(r"=>\s*\{", 4),
            // async/await pattern
            wp!(r"\basync\s+function\b", 4),
            wp!(r"\bawait\s+\w+", 4),
        ],
        hints: &[
            wp!(r"(?m)\bfunction\s+\w*\s*\(", 2),
            wp!(r"(?m)\b(const|let|var)\s+\w+\s*=", 2),
            wp!(r"=>\s*[\{(\n]", 3),
            wp!(r"\bdocument\.\w+", 3),
            wp!(r"\.addEventListener\s*\(", 3),
            wp!(r"===|!==", 2),
            wp!(r"\bwindow\.\w+", 2),
            wp!(r"\.then\s*\(", 2),
            wp!(r"\bPromise\.(all|resolve|reject|allSettled|any|race)\b", 3),
            // JSON.parse / JSON.stringify
            wp!(r"\bJSON\.(parse|stringify)\s*\(", 3),
            // new ClassName()
            wp!(r"\bnew\s+[A-Z]\w+\s*\(", 2),
            // 'use strict' directive — common in JS files
            wp!(r#"(?m)^['"]use strict['"]"#, 3),
        ],
        disqualifiers: &[
            // import type / export type / inline type imports are TS-only syntax
            wp!(r"(?m)^\s*import\s+type\s+", 1),
            wp!(r"(?m)^\s*export\s+type\s*\{", 1),
            wp!(r"import\s*\{[^}]*\btype\s+\w+", 1),
        ],
    }
}