use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition()-> LanguageDefinition {
    LanguageDefinition {
        name: "jinja",
        extensions: &[".j2", ".jinja", ".jinja2"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
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
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Markup],
        anchors: &[
            wp!(r"\{%\s*\w+", 5),
            wp!(r"\{%\s*end\w+", 5),
            wp!(r"\{%\s*extends\s+", 5),
            wp!(r"\{\{.*\}\}", 4),
            wp!(r"\{%\s*block\s+\w+", 4),
            wp!(r"\{%\s*for\s+\w+\s+in\s+", 4),
            wp!(r"\{%\s*macro\s+", 4),
            // Django-specific: {% load %}, {% csrf_token %}, {% url %}
            wp!(r"\{%\s*load\s+", 4),
            wp!(r"\{%\s*csrf_token\s*%\}", 5),
            wp!(r"\{%\s*url\s+", 4),
        ],
        hints: &[
            wp!(r"\{%\s*include\s+", 3),
            wp!(r"\{%\s*if\s+", 3),
            wp!(r"\{%\s*set\s+", 3),
            wp!(r"\{#.*#\}", 3),
            wp!(r"\|\s*(safe|escape|truncatewords|default|length|join|upper|lower)\b", 2),
            // {% elif %} / {% with %} — template control flow
            wp!(r"\{%\s*elif\s+", 3),
            wp!(r"\{%\s*with\s+", 3),
            // {% trans %} — Django i18n
            wp!(r"\{%\s*(trans|blocktrans)\s+", 3),
            // {% static %} — Django static files
            wp!(r"\{%\s*static\s+", 3),
        ],
        disqualifiers: &[],
    }
}
