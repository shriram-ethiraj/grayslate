use super::NamingDefinition;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "svelte",
        extension: "svelte",
        extract: |content| crate::naming::markup::extract_xml_html(content),
    }
}
