use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "ruby",
        extensions: &[".rb"],
        filenames: &["gemfile", "rakefile"],
        filename_patterns: &[],
        shebangs: &[r"\bruby\b"],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"(?m)^\s*def\s+\w+", 3),
            wp!(r"(?m)^\s*end\s*$", 3),
            wp!(r"(?m)^\s*class\s+\w+(\s*<\s*\w+)?", 2),
            wp!(r"(?m)^\s*module\s+\w+", 3),
            wp!(r#"\bputs\s+['"\w]"#, 3),
            wp!(r#"\brequire\s+['""]"#, 3),
            wp!(r#"\brequire_relative\s+['""]"#, 5),
            wp!(r"\battr_(accessor|reader|writer)\s+:", 5),
            wp!(r"\bdo\s*\|[\w,\s]+\|", 4),
            wp!(r"\.(each|map|select|reject|inject|collect)\s*(\{|\bdo\b)", 3),
            wp!(r"\b(nil|true|false)\b", 1),
            wp!(r"@\w+\s*=", 2),
            wp!(r"(?m)^\s*if\s+.*\s*$", 1),
            wp!(r"(?m)^\s*unless\s+", 4),
            wp!(r"\bself\.\w+", 1),
            // Block comments (relevance 10 in highlight.js)
            wp!(r"(?m)^=begin\b", 5),
            // Percent literals: %w[], %i[], %r[]
            wp!(r"%(w|i|r|q|Q|x)\s*[\[\({<]", 4),
            // Symbol literal :name
            wp!(r"(?m)^\s*:\w+\s*=>", 3),
            // Heredoc: <<~HEREDOC
            wp!(r"<<[~\-]?\s*['`]?\w+['`]?", 3),
        ],
        anti_patterns: &[
            wp!(r"(?m);\s*$", -2),
        ],
        uses_hash_comments: true,
        keywords: &[
            "def", "end", "module", "unless", "until", "begin",
            "ensure", "rescue", "raise", "yield", "elsif", "when",
            "undef", "alias", "defined", "redo", "retry",
            "prepend", "include", "extend", "attr_accessor",
            "attr_reader", "attr_writer", "puts", "require_relative",
        ],
        builtins: &[
            "proc", "lambda", "freeze", "frozen", "taint", "untaint",
            "respond_to", "send", "method_missing", "define_method",
            "class_eval", "instance_eval", "module_eval",
            "instance_variable_get", "instance_variable_set",
            "each", "map", "select", "reject", "inject", "collect",
        ],
        family: None,
        exclusive_patterns: &[],
    }
}
