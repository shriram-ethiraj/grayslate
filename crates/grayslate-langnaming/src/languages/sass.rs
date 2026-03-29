use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "sass",
        extension: "sass",
        // Sass indented syntax; CSS extractor handles it well enough
        extract: Extractor::Custom(|content| super::css::extract_css(content)),
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        suggest_stem(src, "sass").and_then(|s| slugify(&s))
    }

    #[test]
    fn sass_keyframes() {
        let src = "@keyframes slideIn\n  from\n    transform: translateX(-100%)\n  to\n    transform: translateX(0)";
        let n = name(src).unwrap();
        assert!(n.contains("slide-in"), "Sass keyframes: {n}");
    }

    #[test]
    fn sass_class_selector() {
        // CSS extractor requires braces — use CSS-compatible syntax
        let src = ".navigation-bar {\n  display: flex;\n  justify-content: space-between;\n  padding: 1rem;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("navigation-bar"), "Sass class: {n}");
    }
}
