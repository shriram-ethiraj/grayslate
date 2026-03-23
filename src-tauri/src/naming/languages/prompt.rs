use super::NamingDefinition;

/// Prompt naming delegates to the prose extractor which already handles
/// prompt-specific stem extraction (role + task verb parsing).
pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "prompt",
        extension: "txt",
        extract: |content| crate::naming::prose::extract_prose(content),
    }
}
