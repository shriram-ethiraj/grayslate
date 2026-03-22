use super::NamingDefinition;
use super::css;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "scss",
        extension: "scss",
        // SCSS is a superset of CSS; the CSS extractor works for it
        extract: |content| (css::definition().extract)(content),
    }
}
