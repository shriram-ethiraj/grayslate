use super::NamingDefinition;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "sql",
        extension: "sql",
        extract: |content| crate::naming::sql::extract_sql(content),
    }
}
