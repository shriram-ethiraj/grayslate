use super::NamingDefinition;
use crate::naming::code::extract_with_regex;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "objectivec",
        extension: "m",
        extract: extract_objc,
    }
}

fn extract_objc(content: &str) -> Option<String> {
    const PATTERNS: &[(&str, u8)] = &[
        (r"(?m)^@interface\s+([A-Z]\w+)", 9),
        (r"(?m)^@implementation\s+([A-Z]\w+)", 9),
        (r"(?m)^@protocol\s+([A-Z]\w+)", 8),
        (r"(?m)^[-+]\s*\([^)]+\)\s*([a-zA-Z_]\w*)", 7),
    ];
    extract_with_regex(content, PATTERNS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objc_interface() {
        let src = "@interface AppDelegate : UIResponder <UIApplicationDelegate>\n@end";
        assert!(extract_objc(src).unwrap().contains("AppDelegate"));
    }
}
