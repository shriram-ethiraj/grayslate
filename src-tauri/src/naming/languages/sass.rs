use super::NamingDefinition;
use super::css;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "sass",
        extension: "sass",
        // Sass indented syntax; CSS extractor handles it well enough
        extract: |content| (css::definition().extract)(content),
    }
}
