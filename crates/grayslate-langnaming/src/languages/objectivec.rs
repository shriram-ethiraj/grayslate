use super::{NamingDefinition, Extractor};
use crate::code::SymbolPattern;

/// Objective-C symbol patterns for declarative extraction.
static PATTERNS: &[SymbolPattern] = &[
    SymbolPattern { regex: r"(?m)^@interface\s+([A-Z]\w+)", priority: 9, capture_group: 1 },
    SymbolPattern { regex: r"(?m)^@implementation\s+([A-Z]\w+)", priority: 9, capture_group: 1 },
    SymbolPattern { regex: r"(?m)^@protocol\s+([A-Z]\w+)", priority: 8, capture_group: 1 },
    SymbolPattern { regex: r"(?m)^[-+]\s*\([^)]+\)\s*([a-zA-Z_]\w*)", priority: 7, capture_group: 1 },
];

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "objectivec",
        extension: "m",
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
    fn objc_interface() {
        let src = "@interface AppDelegate : UIResponder <UIApplicationDelegate>\n@end";
        let n = name(src).unwrap();
        assert!(n.contains("app-delegate"), "got: {n}");
    }

    #[test]
    fn objc_implementation() {
        let src = "@implementation NetworkManager\n- (void)fetchData:(NSString *)url {\n}\n@end";
        let n = name(src).unwrap();
        assert!(n.contains("network-manager"), "got: {n}");
    }

    #[test]
    fn objc_protocol() {
        let src = "@protocol Cacheable <NSObject>\n- (void)invalidateCache;\n@end";
        let n = name(src).unwrap();
        assert!(n.contains("cacheable"), "got: {n}");
    }

    #[test]
    fn objc_integration() {
        let stem = suggest_stem("@interface UserProfile : NSObject\n@property (nonatomic, strong) NSString *name;\n@end", "objectivec").unwrap();
        assert!(stem.contains("user-profile"), "integration: {stem}");
    }

    #[test]
    fn objc_category() {
        let src = "@interface NSString (URLEncoding)\n- (NSString *)urlEncode;\n@end";
        let n = name(src).unwrap();
        assert!(n.contains("string"), "category class extracted: {n}");
    }

    #[test]
    fn objc_method_only() {
        let src = "- (void)viewDidLoad {\n    [super viewDidLoad];\n}";
        let n = name(src).unwrap();
        assert!(n.contains("view-did-load"), "method fallback: {n}");
    }
}
