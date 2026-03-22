use super::NamingDefinition;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "csv",
        extension: "csv",
        extract: |content| crate::naming::structured::extract_csv(content),
    }
}
