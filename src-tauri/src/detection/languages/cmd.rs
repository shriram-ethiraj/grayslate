use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "cmd",
        extensions: &[".bat", ".cmd"],
        filenames: &["autoexec.bat"],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            // `@echo off` / `@echo on` — the canonical Batch file header
            wp!(r"(?mi)^\s*@echo\s+(off|on)\s*$", 6),
            // setlocal / endlocal — Batch-only scoping
            wp!(r"(?mi)^\s*(setlocal|endlocal)\b", 5),
            // `if exist`, `if not`, `if errorlevel`, `if defined` — Batch-specific if forms
            wp!(r"(?mi)^\s*if\s+(exist|not\s+exist|errorlevel|defined|not\s+defined)\b", 4),
            // `for /F`, `for /D`, `for /R`, `for /L` — Batch loop switches
            wp!(r"(?mi)^\s*for\s+/[a-zA-Z]\s+", 4),
            // goto label — very common in batch, unusual elsewhere
            wp!(r"(?mi)^\s*goto\s+\:?\w+", 3),
            // `:label` at start of line — subroutine/jump targets
            wp!(r"(?mi)^\:[a-zA-Z_]\w*\s*$", 3),
            // call :sub or call script.bat
            wp!(r"(?mi)^\s*call\s+", 2),
            // %VAR% or delayed !VAR! variable expansion
            wp!(r"%[A-Za-z_]\w*%", 2),
            wp!(r"![A-Za-z_]\w*!", 2),
            // set /p (user prompt) or set /a (arithmetic) — Batch-specific flags
            wp!(r"(?mi)^\s*set\s+/[pa]\b", 3),
            // rem comment line
            wp!(r"(?mi)^\s*rem\s", 2),
            // :: comment line (Batch-specific comment syntax)
            wp!(r"(?m)^\s*::", 2),
            // Common batch commands
            wp!(r"(?mi)^\s*(echo|pause|cls|dir|del|copy|move|ren|type|xcopy|robocopy)\s", 1),
        ],
        anti_patterns: &[
            // Bash-style `$VAR` references are not Batch
            wp!(r"\$\{?[A-Za-z_]\w*\}?", -4),
            // Bash-style `fi`, `done`, `esac` are not Batch
            wp!(r"(?m)^\s*(fi|done|esac)\s*$", -4),
            // JS/TS console calls
            wp!(r"\bconsole\.\w+\s*\(", -5),
        ],
        uses_hash_comments: false,
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
        family: None,
        exclusive_patterns: &[],
    }
}
