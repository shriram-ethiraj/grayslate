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
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::ShellScript],
        anchors: &[
            // Shebang-like patterns
            wp!(r"(?m)^#!\s*/.*\b(ba|z|k|fi)?sh\b", 5),
            // set -e / set -x
            wp!(r"(?m)^\s*set\s+-[euxo]", 4),
            // export VAR=
            wp!(r"(?m)^\s*export\s+\w+=", 4),
            // $(...) subshell
            wp!(r"\$\(.*\)", 3),
        ],
        hints: &[
            wp!(r#"(?m)^\s*echo\s+["$']"#, 2),
            wp!(r"(?m)^\s*if\s+\[\[?\s", 3),
            wp!(r"(?m)^\s*for\s+\w+\s+in\s+", 3),
            wp!(r"(?m)^\s*case\s+.*\s+in\s*$", 3),
            // Pipe chains
            wp!(r"\|\s*\w+", 2),
        ],
        rivals: &["cmd", "powershell"],
        differentiators: &[
            // #!/bin/bash — not in cmd or powershell
            wp!(r"(?m)^#!\s*/.*\b(ba|z|k)?sh\b", 5),
            // export — Bash, not cmd/powershell
            wp!(r"(?m)^\s*export\s+\w+=", 4),
            // $(...) subshell — not in cmd
            wp!(r"\$\(.*\)", 3),
            // Unix paths
            wp!(r"/usr/(?:bin|local|lib)/", 3),
        ],
        disqualifiers: &[],
    }
}
