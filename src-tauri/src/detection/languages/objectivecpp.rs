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
        patterns: &[
            wp!(r"@interface\s+\w+", 5),
            wp!(r"@implementation\s+\w+", 5),
            wp!(r"@property\s*\(", 4),
            wp!(r"\[\w+\s+\w+[:\]]", 3),
            wp!(r#"#import\s+[<"]"#, 3),
            wp!(r"\bNS\w{3,}\b", 2),
            wp!(r"\bstd::", 2),
            wp!(r"\btemplate\s*<", 2),
            wp!(r"\bnamespace\s+\w+", 2),
        ],
        anti_patterns: &[],
        uses_hash_comments: false,
        keywords: &[
            "@interface", "@implementation", "@protocol", "@property", "@synthesize",
            "@end", "namespace", "template", "typename", "class", "virtual", "override",
        ],
        builtins: &[
            "NSObject", "NSString", "NSArray", "NSDictionary", "NSLog", "std",
            "shared_ptr", "unique_ptr", "vector", "string",
        ],
        family: Some("c-family"),
        exclusive_patterns: &[],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"@interface\s+\w+", 5),
            wp!(r"@implementation\s+\w+", 5),
            wp!(r"@property\s*\(", 4),
        ],
        hints: &[
            wp!(r"\[\w+\s+\w+[:\]]", 3),
            wp!(r#"#import\s+[<"]"#, 3),
            wp!(r"\bNS\w{3,}\b", 2),
            wp!(r"\bstd::", 2),
            wp!(r"\btemplate\s*<", 2),
        ],
        rivals: &["cpp"],
        differentiators: &[
            wp!(r"@interface\s+\w+", 5),
            wp!(r"@implementation\s+\w+", 5),
            wp!(r"@property\s*\(", 4),
            wp!(r"\bNS\w{3,}\b", 3),
        ],
        disqualifiers: &[],
    }
}
