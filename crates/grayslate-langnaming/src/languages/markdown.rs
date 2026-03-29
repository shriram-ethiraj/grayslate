use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "markdown",
        extension: "md",
        extract: Extractor::Custom(|content| crate::markup::extract_markdown(content)),
    }
}
