use super::{wp, LanguageDefinition};

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
        illegal: None,
        extends: None,
    }
}
