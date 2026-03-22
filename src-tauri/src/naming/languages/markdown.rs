use super::NamingDefinition;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "markdown",
        extension: "md",
        extract: |content| crate::naming::markup::extract_markdown(content),
    }
}
