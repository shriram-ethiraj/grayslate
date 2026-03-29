use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "shell",
        extensions: &[".sh", ".bash", ".zsh", ".fish", ".ksh"],
        filenames: &["makefile", "gnumakefile", ".bashrc", ".bash_profile", ".bash_aliases", ".zshrc", ".zprofile", ".profile"],
        filename_patterns: &[],
        shebangs: &[r"\b(ba|z|k|fi)?sh\b"],
        structural_priority: None,
        structural_detect: None,
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
        // ── Family-gated fields ───────────────────────────────
        content_families: &[ContentFamily::ShellScript],
        anchors: &[
            // set -e / set -x
            wp!(r"(?m)^\s*set\s+-[euxo]", 4),
            // export VAR=
            wp!(r"(?m)^\s*export\s+\w+=", 4),
            // $(...) subshell
            wp!(r"\$\(.*\)", 3),
            // fi/done/esac — shell-specific end keywords
            wp!(r"\b(fi|done|esac)\b", 5),
            // case...in
            wp!(r"\bcase\s+.*\bin\b", 5),
            // source command
            wp!(r"(?m)^\s*source\s+", 4),
            // Function declaration: function name() or name()
            wp!(r"(?m)^\s*function\s+\w+\s*\{", 4),
            // local/declare variable
            wp!(r"(?m)^\s*(local|declare)\s+\w+", 4),
        ],
        hints: &[
            wp!(r#"(?m)^\s*echo\s+["$']"#, 2),
            wp!(r"(?m)^\s*if\s+\[\[?\s", 3),
            wp!(r"(?m)^\s*for\s+\w+\s+in\s+", 3),
            // Pipe chains
            wp!(r"\|\s*\w+", 2),
            // Redirections
            wp!(r"[12]?>", 2),
            // Heredoc
            wp!(r"<<[-~]?\w+", 3),
            // Variable expansion ${VAR}
            wp!(r"\$\{\w+", 2),
            // elif/then
            wp!(r"\b(elif|then)\b", 2),
            // test -f / test -d
            wp!(r"\btest\s+-[fdrwxs]\s", 2),
        ],
        disqualifiers: &[
            // PowerShell cmdlets
            wp!(r"(Get|Set|New|Remove)-\w+", -5),
        ],
    }
}
