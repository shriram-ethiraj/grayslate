use super::NamingDefinition;

/// Catch-all definition for unrecognised languages.
/// Falls back to the prose extractor (email → prompt → YAKE keywords).
pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "text",
        extension: "txt",
        extract: |content| crate::naming::prose::extract_prose(content),
    }
}
