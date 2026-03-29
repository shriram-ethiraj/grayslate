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
        // ── Family-gated fields ───────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"\buse\s+strict\b", 5),
            wp!(r"\buse\s+warnings\b", 5),
            // =head1 — Perl POD documentation
            wp!(r"=head1\s+", 5),
            // package Name::Space; — Perl package declaration
            wp!(r"\bpackage\s+[\w:]+\s*;", 5),
            wp!(r"\bmy\s+[\$@%]", 4),
            wp!(r"\bour\s+[\$@%]", 4),
            wp!(r"\bsub\s+\w+\s*\{", 4),
            wp!(r"=~\s*[ms]/", 4),
            wp!(r"\bqw\s*[(\[{/!|]", 4),
            wp!(r"__END__", 4),
            wp!(r"__DATA__", 4),
            // $_ — Perl default variable
            wp!(r"\$_\b", 4),
        ],
        hints: &[
            wp!(r"\buse\s+\w+(::\w+)*\s*;", 3),
            wp!(r"\bchomp\s*[\$@]", 3),
            wp!(r"\bbless\s+\{", 3),
            // foreach my $var — Perl loop idiom
            wp!(r"\bforeach\s+my\s+\$", 3),
            // @_ — Perl subroutine arguments
            wp!(r"@_\b", 3),
            // use Moose / use Moo — Perl OO frameworks
            wp!(r"\buse\s+(Moose|Moo)\b", 3),
            wp!(r#"\b(print|say)\s+[\$@"']"#, 2),
            wp!(r"\bdie\s+", 2),
            // =cut — Perl POD terminator
            wp!(r"=cut\b", 2),
            // shift — Perl argument unpacking
            wp!(r"\bshift\b", 2),
        ],
        disqualifiers: &[],
    }
}
