use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "svelte",
        extension: "svelte",
        extract: Extractor::Custom(|content| crate::markup::extract_xml_html(content)),
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        suggest_stem(src, "svelte").and_then(|s| slugify(&s))
    }

    #[test]
    fn svelte_component_class() {
        let src = "<script>\n  let count = 0;\n</script>\n\n<div class=\"counter-widget\">\n  <button on:click={() => count++}>{count}</button>\n</div>";
        let n = name(src).unwrap();
        assert!(n.contains("counter-widget"), "Svelte class attr: {n}");
    }

    #[test]
    fn svelte_main_layout() {
        let src = "<nav id=\"app-navigation\">\n  <a href=\"/\">Home</a>\n</nav>\n<slot />";
        let n = name(src).unwrap();
        assert!(n.contains("app-navigation"), "Svelte nav id: {n}");
    }
}
