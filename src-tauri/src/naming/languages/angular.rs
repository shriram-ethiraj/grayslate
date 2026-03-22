use super::NamingDefinition;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "angular",
        extension: "angular",
        extract: |content| crate::naming::markup::extract_xml_html(content),
    }
}
