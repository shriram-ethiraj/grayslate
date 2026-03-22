use super::NamingDefinition;
use crate::naming::code::extract_with_regex;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "objectivecpp",
        extension: "mm",
        extract: extract_objcpp,
    }
}

fn extract_objcpp(content: &str) -> Option<String> {
    const PATTERNS: &[(&str, u8)] = &[
        (r"(?m)^namespace\s+([a-zA-Z_]\w*)", 10),
        (r"(?m)^@interface\s+([A-Z]\w+)", 9),
        (r"(?m)^@implementation\s+([A-Z]\w+)", 9),
        (r"(?m)^class\s+([A-Z]\w+)", 9),
        (r"(?m)^@protocol\s+([A-Z]\w+)", 8),
        (r"(?m)^[-+]\s*\([^)]+\)\s*([a-zA-Z_]\w*)", 7),
    ];
    extract_with_regex(content, PATTERNS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objcpp_implementation() {
        let src = "@implementation AudioEngine\n- (void)play {}\n@end";
        assert!(extract_objcpp(src).unwrap().contains("AudioEngine"));
    }
}
