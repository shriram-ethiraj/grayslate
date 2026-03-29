use super::LanguageDefinition;
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "text",
        extensions: &[".ini", ".cfg", ".env", ".bat", ".cmd", ".lua"],
        filenames: &[
            ".gitignore", ".gitattributes", ".env", ".env.local",
            ".npmrc", ".eslintignore", ".prettierignore",
            ".dockerignore", ".gitkeep",
            "jenkinsfile", "vagrantfile",
        ],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        keywords: &[],
        builtins: &[],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Prose],
        anchors: &[],
        hints: &[],
        disqualifiers: &[],
    }
}
