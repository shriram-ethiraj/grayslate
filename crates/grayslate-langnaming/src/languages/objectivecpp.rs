use super::{NamingDefinition, Extractor};
use crate::code::SymbolPattern;

/// Objective-C++ patterns: union of C++ and Objective-C constructs.
static PATTERNS: &[SymbolPattern] = &[
    SymbolPattern { regex: r"(?m)^@interface\s+([A-Z]\w+)", priority: 9, capture_group: 1 },
    SymbolPattern { regex: r"(?m)^@implementation\s+([A-Z]\w+)", priority: 9, capture_group: 1 },
    SymbolPattern { regex: r"(?m)^(?:template\s*<[^>]*>\s*)?class\s+([A-Z]\w+)", priority: 9, capture_group: 1 },
    SymbolPattern { regex: r"(?m)^@protocol\s+([A-Z]\w+)", priority: 8, capture_group: 1 },
    SymbolPattern { regex: r"(?m)^[-+]\s*\([^)]+\)\s*([a-zA-Z_]\w*)", priority: 7, capture_group: 1 },
    SymbolPattern { regex: r"(?m)^namespace\s+([a-zA-Z_]\w*)", priority: 5, capture_group: 1 },
];

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "objectivecpp",
        extension: "mm",
        extract: Extractor::Patterns { symbols: PATTERNS, noise: &[] },
    }
}

#[cfg(test)]
mod tests {
    use crate::code::extract_from_patterns;
    use crate::shared::slugify;
    use crate::suggest_stem;
    use super::PATTERNS;

    fn name(src: &str) -> Option<String> {
        extract_from_patterns(src, PATTERNS, &[]).and_then(|s| slugify(&s))
    }

    #[test]
    fn objcpp_implementation() {
        let src = "@implementation AudioEngine\n- (void)play {}\n@end";
        let n = name(src).unwrap();
        assert!(n.contains("audio-engine"), "got: {n}");
    }

    #[test]
    fn objcpp_class_beats_namespace() {
        let src = "namespace audio {\nclass AudioProcessor {\npublic:\n    void process();\n};\n}";
        let n = name(src).unwrap();
        assert!(n.contains("audio-processor"), "class beats namespace: {n}");
    }

    #[test]
    fn objcpp_mixed_cpp_and_objc() {
        let src = "@interface AudioBridge : NSObject\n@end\n\nnamespace engine {\nclass AudioCore {};\n}";
        let n = name(src).unwrap();
        assert!(n.contains("audio"), "mixed: {n}");
    }

    #[test]
    fn objcpp_protocol() {
        let src = "@protocol Renderable <NSObject>\n- (void)render;\n@end";
        let n = name(src).unwrap();
        assert!(n.contains("renderable"), "protocol: {n}");
    }

    #[test]
    fn objcpp_integration() {
        let stem = suggest_stem("@implementation VideoDecoder\n- (void)decode {}\n@end", "objectivecpp").unwrap();
        assert!(stem.contains("video-decoder"), "integration: {stem}");
    }
}
