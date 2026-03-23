use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition()-> LanguageDefinition {
    LanguageDefinition {
        name: "perl",
        extensions: &[".pl", ".pm", ".perl", ".pod", ".t"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[r"\bperl\b"],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            // Sigil variables — the most distinctive Perl signal.
            wp!(r"\$\w+", 2),
            wp!(r"@\w+", 2),
            wp!(r"%\w+", 2),
            // Variable declarations and sub definitions.
            wp!(r"\bmy\s+[\$@%]", 4),
            wp!(r"\bour\s+[\$@%]", 4),
            wp!(r"\bsub\s+\w+\s*\{", 4),
            // use/require statements.
            wp!(r"\buse\s+strict\b", 5),
            wp!(r"\buse\s+warnings\b", 5),
            wp!(r"\buse\s+\w+(::\w+)*\s*;", 3),
            // Regex operators (=~ m// and =~ s///).
            wp!(r"=~\s*[ms]/", 4),
            wp!(r"\bqw\s*[(\[{/!|]", 4),
            // print/say with a Perl-style argument (sigil, string, or filehandle).
            // "print something" is too generic — require $var, @arr, "str", or 'str'.
            wp!(r#"\b(print|say)\s+[\$@"']"#, 2),
            // Perl-specific string ops.
            wp!(r"\bchomp\s*[\$@]", 3),
            wp!(r"\bdie\s+", 2),
            wp!(r"__END__", 4),
            wp!(r"__DATA__", 4),
            // Arrow method call: $obj->method
            wp!(r"\$\w+->", 2),
            // OO: bless, ref
            wp!(r"\bbless\s+\{", 3),
        ],
        anti_patterns: &[
            wp!(r"(?m)^\s*def\s+\w+\s*[\(:]", -3), // Python/Ruby def
            wp!(r"(?m)^\s*fn\s+\w+\s*\(", -4),      // Rust fn
            wp!(r"(?m)^\s*func\s+\w+\s*\(", -3),    // Go func
        ],
        uses_hash_comments: true,
        keywords: &[
            "my", "our", "local", "sub", "use", "require", "package",
            "if", "elsif", "else", "unless", "while", "until", "for",
            "foreach", "do", "last", "next", "redo", "return", "undef",
            "die", "warn", "print", "say", "push", "pop", "shift",
            "unshift", "wantarray", "bless", "ref", "scalar", "defined",
            "chomp", "chop", "keys", "values", "each", "grep", "map",
            "sort", "reverse", "splice", "join", "split", "qw", "qq",
            "eval", "BEGIN", "END",
        ],
        builtins: &[
            "STDIN", "STDOUT", "STDERR", "ARGV", "ENV",
            "Carp", "Exporter", "Scalar::Util", "List::Util",
            "File::Basename", "File::Path", "Data::Dumper",
            "Getopt::Long", "POSIX", "DBI", "CGI", "LWP",
        ],
        family: None,
        exclusive_patterns: &[],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"\buse\s+strict\b", 5),
            wp!(r"\buse\s+warnings\b", 5),
            wp!(r"\bmy\s+[\$@%]", 4),
            wp!(r"\bour\s+[\$@%]", 4),
            wp!(r"\bsub\s+\w+\s*\{", 4),
            wp!(r"=~\s*[ms]/", 4),
            wp!(r"\bqw\s*[(\[{/!|]", 4),
            wp!(r"__END__", 4),
            wp!(r"__DATA__", 4),
        ],
        hints: &[
            wp!(r"\buse\s+\w+(::\w+)*\s*;", 3),
            wp!(r"\bchomp\s*[\$@]", 3),
            wp!(r"\bbless\s+\{", 3),
            wp!(r#"\b(print|say)\s+[\$@"']"#, 2),
            wp!(r"\bdie\s+", 2),
        ],
        rivals: &["ruby", "python"],
        differentiators: &[
            wp!(r"\buse\s+strict\b", 5),
            wp!(r"\buse\s+warnings\b", 5),
            wp!(r"\bmy\s+[\$@%]", 4),
            wp!(r"=~\s*[ms]/", 4),
            wp!(r"\$\w+->", 3),
            wp!(r"__END__", 4),
        ],
        disqualifiers: &[],
    }
}
