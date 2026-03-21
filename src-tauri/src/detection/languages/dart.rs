use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "dart",
        extensions: &[".dart"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r#"(?m)^\s*import\s+['"]package:"#, 5),
            wp!(r"(?m)^\s*void\s+main\s*\(\)\s*(async\s*)?\{", 3),
            wp!(r"\bWidget\s+build\s*\(", 5),
            wp!(r"\b(StatelessWidget|StatefulWidget|State<\w+>)\b", 5),
            wp!(r"\bfinal\s+\w+\s*=", 2),
            wp!(r"\bvar\s+\w+\s*=", 1),
            wp!(r"\blate\s+(final\s+)?\w+\s+\w+", 4),
            wp!(r"\b@override\b", 2),
            wp!(r"\brequired\s+this\.\w+", 4),
            wp!(r"(?m)\bclass\s+\w+\s*extends\s+\w+", 1),
            wp!(r"\bFuture<\w+>", 3),
            wp!(r"\basync\s*\*", 2),
            wp!(r"\bprint\s*\(", 1),
            wp!(r"\b(List|Map|Set|String|int|double|bool|dynamic)\b", 1),
        ],
        anti_patterns: &[
            wp!(r"\bprintln!\s*\(", -5),
        ],
        uses_hash_comments: false,
        keywords: &[
            "var", "final", "late", "required", "covariant", "deferred",
            "factory", "external", "part", "library", "export",
            "on", "show", "hide", "sync", "async", "yield",
            "rethrow", "assert", "typedef", "mixin", "with",
            "extension", "abstract", "sealed", "base", "get", "set",
        ],
        builtins: &[
            "print", "int", "double", "bool", "string", "list",
            "map", "set", "future", "stream", "widget",
            "statefulwidget", "statelesswidget", "buildcontext",
            "scaffold", "appbar", "column", "row", "center",
            "container", "text", "edgeinsetsall", "sizedbox",
            "navigator", "materialapp", "themedata",
        ],
        illegal: None,
        extends: None,
    }
}
