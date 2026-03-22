use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "shell",
        extensions: &[".sh", ".bash", ".zsh", ".fish", ".ksh"],
        filenames: &["makefile", "gnumakefile", ".bashrc", ".bash_profile", ".bash_aliases", ".zshrc", ".zprofile", ".profile"],
        filename_patterns: &[],
        shebangs: &[r"\b(ba|z|k|fi)?sh\b"],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r#"(?m)^\s*echo\s+["$']"#, 2),
            wp!(r"(?m)^\s*if\s+\[\[?\s", 3),
            wp!(r"(?m)^\s*fi\s*$", 5),
            wp!(r"(?m)^\s*done\s*$", 4),
            wp!(r"(?m)^\s*esac\s*$", 5),
            wp!(r"(?m)^\s*export\s+\w+=", 3),
            wp!(r"\$\{[\w?!#@*+\-]+", 2),
            wp!(r"\$\(.*\)", 2),
            wp!(r"(?m)^\s*case\s+.*\s+in\s*$", 3),
            wp!(r"(?m)^\s*(alias|source|chmod|mkdir|rm\s|cp\s|mv\s|cd\s|grep|sed|awk)\s", 2),
            wp!(r#"<<-?\s*['"]?\w+['"]?"#, 3),
        ],
        anti_patterns: &[
            wp!(r"\bconsole\.\w+\s*\(", -5),
        ],
        uses_hash_comments: true,
        keywords: &[
            "fi", "done", "esac", "elif", "then", "declare", "local",
            "readonly", "unset", "select", "until", "coproc", "function",
            "export", "source", "alias",
        ],
        builtins: &[
            "echo", "printf", "test", "getopts", "pushd", "popd",
            "dirs", "mapfile", "readarray", "compgen", "complete",
            "builtin", "command", "typeset", "ulimit", "umask",
        ],
        family: None,
        exclusive_patterns: &[],
    }
}
