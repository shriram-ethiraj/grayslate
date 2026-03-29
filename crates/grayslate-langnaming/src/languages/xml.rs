use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "xml",
        extension: "xml",
        extract: Extractor::Custom(|content| crate::markup::extract_xml_html(content)),
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        suggest_stem(src, "xml").and_then(|s| slugify(&s))
    }

    #[test]
    fn xml_root_with_id() {
        let src = r#"<?xml version="1.0"?><project id="analytics-service"><module>core</module></project>"#;
        let n = name(src).unwrap();
        assert!(n.contains("project"), "root element: {n}");
    }

    #[test]
    fn xml_pom_like() {
        let src = "<project>\n  <modelVersion>4.0.0</modelVersion>\n  <artifactId>payment-gateway</artifactId>\n</project>";
        let n = name(src).unwrap();
        assert!(n.contains("project"), "pom root: {n}");
    }
}
