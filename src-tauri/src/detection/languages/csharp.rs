use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "csharp",
        extensions: &[".cs"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"(?m)^\s*using\s+System(\.\w+)*\s*;", 5),
            wp!(r"(?m)^\s*namespace\s+\w+(\.\w+)*", 3),
            wp!(r"(?m)\bpublic\s+(class|struct|interface|enum)\s+\w+", 2),
            wp!(r"\bstatic\s+void\s+Main\s*\(", 5),
            wp!(r"\bConsole\.(Write|WriteLine|ReadLine)\s*\(", 5),
            wp!(r"\bvar\s+\w+\s*=\s*new\s+", 2),
            wp!(r"\basync\s+Task\b", 4),
            wp!(r"\bawait\s+\w+", 1),
            wp!(r"\bstring\.\w+", 2),
            wp!(r"(?m)\b(get|set)\s*[;\{]", 2),
            wp!(r"\bLINQ|\.Select\(|\.Where\(|\.OrderBy\(", 4),
            wp!(r"(?m)^\s*\[[\w.]+(\(.*)?\]\s*$", 2),
            wp!(r"\b(IEnumerable|IList|IDictionary|IQueryable)<", 3),
        ],
        anti_patterns: &[
            wp!(r"\bprintln!\s*\(", -5),
            wp!(r":=\s", -3),
        ],
        uses_hash_comments: false,
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
        illegal: None,
        extends: None,
    }
}
