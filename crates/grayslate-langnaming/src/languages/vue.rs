use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "vue",
        extension: "vue",
        extract: Extractor::Custom(|content| crate::markup::extract_xml_html(content)),
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        suggest_stem(src, "vue").and_then(|s| slugify(&s))
    }

    #[test]
    fn vue_template_with_id() {
        let src = "<template>\n  <div id=\"user-dashboard\">\n    <h1>Dashboard</h1>\n  </div>\n</template>\n<script>\nexport default { name: 'UserDashboard' };\n</script>";
        let n = name(src).unwrap();
        assert!(n.contains("user-dashboard"), "Vue template id: {n}");
    }

    #[test]
    fn vue_root_class() {
        let src = "<template>\n  <section class=\"checkout-form\">\n    <input />\n  </section>\n</template>";
        let n = name(src).unwrap();
        assert!(n.contains("checkout-form"), "Vue root class: {n}");
    }
}
