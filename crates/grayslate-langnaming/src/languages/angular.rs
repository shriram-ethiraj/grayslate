use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "angular",
        extension: "angular",
        extract: Extractor::Custom(|content| crate::markup::extract_xml_html(content)),
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        suggest_stem(src, "angular").and_then(|s| slugify(&s))
    }

    #[test]
    fn angular_component_template() {
        let src = "<div class=\"user-dashboard\">\n  <h2>Welcome</h2>\n  <app-sidebar></app-sidebar>\n</div>";
        let n = name(src).unwrap();
        assert!(n.contains("user-dashboard"), "class attr: {n}");
    }

    #[test]
    fn angular_with_title() {
        let src = "<section title=\"Settings Panel\">\n  <form>\n    <input name=\"username\" />\n  </form>\n</section>";
        let n = name(src).unwrap();
        assert!(n.contains("settings-panel"), "title attr: {n}");
    }
}
