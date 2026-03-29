use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "csv",
        extension: "csv",
        extract: Extractor::Custom(|content| crate::structured::extract_csv(content)),
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;

    fn name(src: &str) -> Option<String> {
        suggest_stem(src, "csv")
    }

    #[test]
    fn csv_header_extraction() {
        let src = "name,email,department\nAlice,alice@co.com,Engineering\nBob,bob@co.com,Sales";
        let n = name(src).unwrap();
        assert!(n.contains("name"), "semantic column: {n}");
    }

    #[test]
    fn csv_noise_columns_filtered() {
        let src = "id,created_at,product,updated_at\n1,2026-01-01,Widget,2026-03-01";
        let n = name(src).unwrap();
        assert!(n.contains("product"), "noise filtered: {n}");
        assert!(!n.contains("created"), "timestamp excluded: {n}");
    }
}
