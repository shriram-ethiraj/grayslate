use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "json",
        extension: "json",
        extract: Extractor::Custom(|content| crate::structured::extract_json(content)),
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;

    fn name(src: &str) -> Option<String> {
        suggest_stem(src, "json")
    }

    #[test]
    fn json_package_name() {
        let src = r#"{"name": "my-awesome-lib", "version": "1.0.0", "main": "index.js"}"#;
        let n = name(src).unwrap();
        assert!(n.contains("my-awesome-lib"), "package name: {n}");
    }

    #[test]
    fn json_array_of_objects() {
        let src = r#"[{"user": "alice", "role": "admin"}, {"user": "bob", "role": "viewer"}]"#;
        let n = name(src).unwrap();
        assert!(n.contains("user"), "array key: {n}");
    }
}
