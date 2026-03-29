use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition()-> LanguageDefinition {
    LanguageDefinition {
        name: "objectivecpp",
        extensions: &[".mm"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        keywords: &[
            "@interface", "@implementation", "@protocol", "@property", "@synthesize",
            "@end", "namespace", "template", "typename", "class", "virtual", "override",
        ],
        builtins: &[
            "NSObject", "NSString", "NSArray", "NSDictionary", "NSLog", "std",
            "shared_ptr", "unique_ptr", "vector", "string",
        ],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            // ObjC++ REQUIRES both ObjC and C++ signals to be present.
            // ObjC-specific patterns (never appear in pure C/C++):
            wp!(r"@interface\s+\w+", 5),
            wp!(r"@implementation\s+\w+", 5),
            wp!(r"@property\s*\(", 4),
            wp!(r"(?m)^@end\b", 4),
            // ObjC method declaration: - (void)method or + (id)method
            wp!(r"(?m)^[-+]\s*\([^)]+\)\s*\w+", 5),
            // #import is ObjC-specific (C/C++ use #include)
            wp!(r#"#import\s+[<"]"#, 4),
        ],
        hints: &[
            // ObjC message passing: [obj method:]
            wp!(r"\[\w+\s+\w+[:\]]", 3),
            wp!(r"\bNS\w{3,}\b", 2),
            // @selector, @autoreleasepool
            wp!(r"@(selector|autoreleasepool)\b", 3),
            // ObjC string literal: @"string"
            wp!(r#"@""#, 2),
        ],
        disqualifiers: &[
            // Files with C/C++ header guards and NO ObjC signals are pure C/C++.
            // The #ifndef/#define pair is the most reliable indicator of C/C++ headers.
            // ObjC++ files use #import, not #include/#ifndef.
        ],
    }
}
