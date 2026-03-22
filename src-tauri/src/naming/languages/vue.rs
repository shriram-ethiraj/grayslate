use super::NamingDefinition;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "vue",
        extension: "vue",
        extract: |content| crate::naming::markup::extract_xml_html(content),
    }
}
