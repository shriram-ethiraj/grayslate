use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "cmd",
        extensions: &[".bat", ".cmd"],
        filenames: &["autoexec.bat"],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        keywords: &[
            "if", "else", "for", "in", "do", "goto", "call", "exit",
            "setlocal", "endlocal", "set", "echo", "pause", "rem",
            "cls", "dir", "cd", "del", "copy", "move", "ren", "type",
            "not", "exist", "defined", "errorlevel", "equ", "neq",
            "lss", "leq", "gtr", "geq",
        ],
        builtins: &[
            "echo", "set", "for", "if", "goto", "call", "pause", "exit",
            "cls", "dir", "cd", "del", "copy", "move", "ren", "type",
            "findstr", "find", "more", "sort", "tasklist", "taskkill",
            "xcopy", "robocopy", "mklink", "attrib", "icacls", "net",
            "sc", "reg", "wmic", "powershell", "cmd",
        ],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::ShellScript],
        anchors: &[
            // @echo off — canonical Batch header
            wp!(r"(?mi)^\s*@echo\s+(off|on)\s*$", 6),
            // %VAR% variable expansion
            wp!(r"%[A-Za-z_]\w*%", 4),
            // set /p (user prompt) or set /a (arithmetic)
            wp!(r"(?mi)^\s*set\s+/[pa]\b", 4),
            // if exist — Batch-specific
            wp!(r"(?mi)^\s*if\s+(exist|not\s+exist|errorlevel|defined)\b", 4),
            // setlocal / endlocal — Batch-only scoping
            wp!(r"(?mi)^\s*(setlocal|endlocal)\b", 5),
            // for /F /D /R /L — Batch loop switches
            wp!(r"(?mi)^\s*for\s+/[a-zA-Z]\s+", 4),
            // goto :label — Batch jump
            wp!(r"(?mi)^\s*goto\s+:?\w+", 4),
        ],
        hints: &[
            wp!(r"(?mi)^\s*echo\.\s*$", 2),
            wp!(r"(?mi)^\s*rem\s", 2),
            // :label at start of line
            wp!(r"(?mi)^\:[a-zA-Z_]\w*\s*$", 3),
            // Windows paths with backslash drive letter
            wp!(r"[A-Z]:\\", 3),
            // :: comment — Batch-specific comment syntax
            wp!(r"(?m)^\s*::", 2),
            // call :sub or call script.bat
            wp!(r"(?mi)^\s*call\s+", 2),
            // Delayed expansion !VAR!
            wp!(r"![A-Za-z_]\w*!", 2),
            // Batch comparison operators: EQU, NEQ, LSS, LEQ, GTR, GEQ
            wp!(r"\b(EQU|NEQ|LSS|LEQ|GTR|GEQ)\b", 3),
        ],
        disqualifiers: &[
            // Bash $VAR / ${VAR} — not Batch
            wp!(r"\$\{?[A-Za-z_]\w*\}?", -4),
            // PowerShell cmdlets — not Batch
            wp!(r"\b(Get|Set|New|Remove)-\w+", -4),
        ],
    }
}
