use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "powershell",
        extensions: &[".ps1", ".psd1", ".psm1"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        keywords: &[
            "param", "process", "begin", "end", "filter", "function",
            "trap", "data", "dynamicparam", "hidden", "static",
            "elseif", "foreach", "until",
        ],
        builtins: &[
            "cmdletbinding", "parameter", "validateset", "validaterange",
            "validatepattern", "validatenotnull", "validatescript",
            "outputtype", "alias",
        ],
        // ── Family-gated fields ───────────────────────────────
        content_families: &[ContentFamily::ShellScript],
        anchors: &[
            wp!(r"\$PSVersionTable\b", 5),
            // Cmdlet patterns: Get-/Set-/New-/Remove-
            wp!(r"\b(Get|Set|New|Remove|Invoke)-\w+", 5),
            // param( block
            wp!(r"(?m)\bparam\s*\(", 4),
            // [CmdletBinding()]
            wp!(r"\[CmdletBinding\(\)\]", 5),
            // [Parameter()] attribute
            wp!(r"\[Parameter\(", 4),
            // $_ pipeline variable property access
            wp!(r"\$_\.", 4),
        ],
        hints: &[
            wp!(r"\$_\b", 3),
            wp!(r"\bWrite-(Host|Output|Error|Verbose|Warning)\b", 3),
            wp!(r"\bForEach-Object\b", 3),
            // Pipe with cmdlets
            wp!(r"\|\s*(Where-Object|ForEach-Object|Select-Object|Sort-Object)\b", 3),
            // Comparison operators
            wp!(r"-eq\b|-ne\b|-gt\b|-lt\b", 3),
            // foreach loop
            wp!(r"foreach\s*\(", 2),
            // try/catch block
            wp!(r"try\s*\{", 1),
            // Type casting
            wp!(r"\[string\]|\[int\]", 2),
            // Environment variable
            wp!(r"\$env:", 3),
        ],
        disqualifiers: &[
            // Bash end keywords
            wp!(r"\b(fi|done|esac)\b", -5),
            // CMD @echo off
            wp!(r"(?i)@echo\s+off", -5),
        ],
    }
}
