use super::NamingDefinition;

/// Email naming delegates to the prose extractor which already handles
/// email-specific stem extraction (subject lines, greeting-based fallback).
pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "email",
        extension: "txt",
        extract: |content| crate::naming::prose::extract_prose(content),
    }
}
