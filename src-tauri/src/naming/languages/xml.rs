use super::NamingDefinition;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "xml",
        extension: "xml",
        extract: |content| crate::naming::markup::extract_xml_html(content),
    }
}
