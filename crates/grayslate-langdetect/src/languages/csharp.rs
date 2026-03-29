use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition()-> LanguageDefinition {
    LanguageDefinition {
        name: "csharp",
        extensions: &[".cs"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        keywords: &[
            "namespace", "using", "partial", "sealed", "virtual",
            "override", "abstract", "delegate", "event", "async",
            "await", "ref", "out", "params", "readonly", "volatile",
            "extern", "unsafe", "fixed", "checked", "unchecked",
            "lock", "stackalloc", "sizeof", "typeof", "nameof",
            "dynamic", "is", "var", "record", "init", "required",
            "get", "set", "value", "add", "remove", "global",
            "where", "select", "orderby", "descending", "ascending",
            "join", "group", "into", "equals", "on", "by", "let",
        ],
        builtins: &[
            "console", "task", "ienumerable", "ilist", "idictionary",
            "iqueryable", "iobservable", "icollection", "icomparable",
            "idisposable", "stringbuilder", "datetime", "timespan",
            "guid", "convert", "activator", "attribute",
        ],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"(?m)^\s*using\s+System(\.\w+)*\s*;", 5),
            wp!(r"\bstatic\s+void\s+Main\s*\(", 5),
            wp!(r"\bConsole\.(Write|WriteLine|ReadLine)\s*\(", 5),
            wp!(r"\basync\s+Task\b", 4),
            wp!(r"\bLINQ|\.Select\(|\.Where\(|\.OrderBy\(", 4),
            // .NET attributes (ASP.NET, serialization, etc.)
            wp!(r"(?m)^\s*\[(HttpGet|HttpPost|HttpPut|HttpDelete|Route|Authorize|ApiController|Serializable)\b", 4),
            // Property accessor shorthand: { get; set; }
            wp!(r"\{\s*get;\s*set;\s*\}", 4),
            // Preprocessor region directive
            wp!(r"(?m)^\s*#region\b", 4),
        ],
        hints: &[
            wp!(r"(?m)^\s*namespace\s+\w+(\.\w+)*", 3),
            wp!(r"(?m)\bpublic\s+(class|struct|interface|enum)\s+\w+", 2),
            wp!(r"\bvar\s+\w+\s*=\s*new\s+", 2),
            wp!(r"\bstring\.\w+", 2),
            wp!(r"\b(IEnumerable|IList|IDictionary|IQueryable)<", 3),
            wp!(r"(?m)\b(get|set)\s*[;\{]", 3),
            wp!(r"\bnameof\s*\(", 3),
            wp!(r"\byield\s+return\b", 3),
            wp!(r"\?\?\s", 2),
            wp!(r"\?\.\w+", 2),
        ],
        disqualifiers: &[
            // Rust
            wp!(r"\bprintln!\s*\(", -5),
            wp!(r"(?m)^\s*(pub\s+)?(fn|mod|trait|impl)\s", -5),
            // Go
            wp!(r":=\s", -4),
            wp!(r"(?m)^\s*func\s+\w+\s*\(", -4),
        ],
    }
}
