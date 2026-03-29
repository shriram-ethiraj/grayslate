use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "scss",
        extension: "scss",
        // SCSS is a superset of CSS; the CSS extractor works for it
        extract: Extractor::Custom(|content| super::css::extract_css(content)),
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        suggest_stem(src, "scss").and_then(|s| slugify(&s))
    }

    #[test]
    fn scss_variable_and_class() {
        let src = "$primary-color: #3498db;\n\n.card-header {\n  background: $primary-color;\n  padding: 1rem;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("card-header"), "class selector through CSS extractor: {n}");
    }

    #[test]
    fn scss_id_selector() {
        let src = "$spacing: 8px;\n\n#checkout-form {\n  display: grid;\n  gap: $spacing;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("checkout-form"), "SCSS ID selector: {n}");
    }
}
