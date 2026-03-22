use super::NamingDefinition;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "toml",
        extension: "toml",
        extract: |content| crate::naming::structured::extract_toml(content),
    }
}
