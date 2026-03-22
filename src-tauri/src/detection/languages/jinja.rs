use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "jinja",
        extensions: &[".j2", ".jinja", ".jinja2"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"\{%\s*\w+", 5),
            wp!(r"\{%\s*end\w+", 5),
            wp!(r"\{\{.*\}\}", 4),
            wp!(r"\{%\s*block\s+\w+", 4),
            wp!(r"\{%\s*extends\s+", 5),
            wp!(r"\{%\s*include\s+", 3),
            wp!(r"\{%\s*for\s+\w+\s+in\s+", 4),
            wp!(r"\{%\s*if\s+", 3),
            wp!(r"\{%\s*macro\s+", 4),
            wp!(r"\{%\s*set\s+", 3),
            wp!(r"\{#.*#\}", 3),
            wp!(r"\|\s*(safe|escape|truncatewords|default|length|join|upper|lower)\b", 2),
        ],
        anti_patterns: &[
            wp!(r"^\s*<\?php", -5),
            wp!(r"(?m)^\s*def\s+\w+\s*\(", -3),
        ],
        uses_hash_comments: false,
        keywords: &[
            "block", "endblock", "extends", "include", "macro", "endmacro",
            "call", "endcall", "filter", "endfilter", "set", "for", "endfor",
            "if", "elif", "else", "endif", "with", "endwith", "autoescape",
            "endautoescape", "raw", "endraw", "trans", "endtrans", "pluralize",
            "spaceless", "endspaceless", "verbatim", "load", "csrf_token",
            "static", "url", "blocktrans", "endblocktrans",
        ],
        builtins: &[
            "safe", "escape", "truncatewords", "default", "length", "join",
            "upper", "lower", "title", "capitalize", "first", "last", "sort",
            "reverse", "batch", "round", "int", "float", "string", "list",
            "dictsort", "filesizeformat", "wordcount", "striptags", "urlencode",
            "center", "truncate",
        ],
        family: None,
        exclusive_patterns: &[],
    }
}
