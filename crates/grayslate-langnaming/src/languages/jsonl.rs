use super::{Extractor, NamingDefinition};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "jsonl",
        extension: "jsonl",
        extract: Extractor::Custom(crate::structured::extract_jsonl),
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;

    #[test]
    fn uses_first_record_for_stem() {
        let content = concat!(
            "{\"name\": \"Alice\", \"age\": 30}\n",
            "{\"name\": \"Bob\", \"age\": 25}\n",
        );
        assert_eq!(suggest_stem(content, "jsonl").as_deref(), Some("alice"));
    }
}
