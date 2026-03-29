use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "sql",
        extension: "sql",
        extract: Extractor::Custom(|content| crate::sql::extract_sql(content)),
    }
}
