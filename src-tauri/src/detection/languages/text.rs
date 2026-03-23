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
        patterns: &[],
        anti_patterns: &[],
        uses_hash_comments: false,
        keywords: &[],
        builtins: &[],
        family: None,
        exclusive_patterns: &[],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Prose],
        anchors: &[],
        hints: &[],
        rivals: &[],
        differentiators: &[],
        disqualifiers: &[],
    }
}
