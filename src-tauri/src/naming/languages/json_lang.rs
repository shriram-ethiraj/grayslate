use super::NamingDefinition;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "json",
        extension: "json",
        extract: |content| crate::naming::structured::extract_json(content),
    }
}
