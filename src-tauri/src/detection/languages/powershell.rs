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
        patterns: &[
            wp!(r"(?m)^\s*function\s+\w+-\w+", 5),
            wp!(r"\$PSVersionTable\b", 5),
            wp!(r"\$_\b", 3),
            wp!(r"\$\w+\s*=", 2),
            wp!(r"\bGet-\w+", 4),
            wp!(r"\bSet-\w+", 4),
            wp!(r"\bNew-\w+", 4),
            wp!(r"\bInvoke-\w+", 4),
            wp!(r"\bWrite-(Host|Output|Error|Verbose|Warning)\b", 5),
            wp!(r"\|\s*(Where-Object|ForEach-Object|Select-Object|Sort-Object)\b", 5),
            wp!(r"(?m)\bparam\s*\(", 3),
            wp!(r"\[CmdletBinding\(\)\]", 5),
            wp!(r"\[Parameter\s*\(", 4),
            wp!(r"\b-eq\b|-ne\b|-gt\b|-lt\b|-ge\b|-le\b", 3),
        ],
        anti_patterns: &[
            wp!(r"\bconsole\.\w+\s*\(", -5),
        ],
        uses_hash_comments: false,
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
        family: None,
        exclusive_patterns: &[],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::ShellScript],
        anchors: &[
            wp!(r"\$PSVersionTable\b", 5),
            // Cmdlet patterns: Get-/Set-/New-/Remove-
            wp!(r"\b(Get|Set|New|Remove|Invoke)-\w+", 5),
            // param( block
            wp!(r"(?m)\bparam\s*\(", 4),
            // [CmdletBinding()]
            wp!(r"\[CmdletBinding\(\)\]", 5),
        ],
        hints: &[
            wp!(r"\$_\b", 3),
            wp!(r"\bWrite-(Host|Output|Error|Verbose|Warning)\b", 3),
            wp!(r"\bForEach-Object\b", 3),
            // Pipe with cmdlets
            wp!(r"\|\s*(Where-Object|ForEach-Object|Select-Object|Sort-Object)\b", 3),
        ],
        rivals: &["shell", "cmd"],
        differentiators: &[
            // Cmdlet patterns — not in shell or cmd
            wp!(r"\b(Get|Set|New|Remove|Invoke)-\w+", 5),
            // $PSVersionTable — PowerShell-only
            wp!(r"\$PSVersionTable\b", 5),
            // param( — PowerShell-only
            wp!(r"(?m)\bparam\s*\(", 4),
            // PascalCase commands (Write-Host etc.)
            wp!(r"\bWrite-(Host|Output|Error|Verbose|Warning)\b", 4),
        ],
        disqualifiers: &[],
    }
}
