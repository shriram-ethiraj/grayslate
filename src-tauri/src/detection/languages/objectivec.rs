use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition()-> LanguageDefinition {
    LanguageDefinition {
        name: "objectivec",
        extensions: &[".m"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"@interface\s+\w+", 5),
            wp!(r"@implementation\s+\w+", 5),
            wp!(r"@protocol\s+\w+", 4),
            wp!(r"@property\s*\(", 4),
            wp!(r"@synthesize\s+\w+", 4),
            wp!(r"@dynamic\s+\w+", 3),
            wp!(r"\[\w+\s+\w+[:\]]", 3),
            wp!(r"@selector\s*\(", 3),
            wp!(r"@autoreleasepool\s*\{", 3),
            wp!(r#"#import\s+[<"]"#, 3),
            wp!(r"\bNS\w{3,}\b", 2),
            wp!(r"\b(YES|NO)\b", 2),
            wp!(r#"@""#, 2),
        ],
        anti_patterns: &[
            wp!(r"\bclass\s+\w+\s*[:{]", -2),
            wp!(r"\bnamespace\s+", -3),
        ],
        uses_hash_comments: false,
        keywords: &[
            "@interface", "@implementation", "@protocol", "@property", "@synthesize",
            "@dynamic", "@selector", "@autoreleasepool", "@end", "@try", "@catch",
            "@finally", "@throw", "nonatomic", "strong", "weak", "copy", "retain",
            "assign", "readonly", "readwrite", "instancetype",
        ],
        builtins: &[
            "NSObject", "NSString", "NSArray", "NSDictionary", "NSMutableArray",
            "NSMutableDictionary", "NSNumber", "NSLog", "NSError",
            "NSNotificationCenter", "NSUserDefaults", "NSBundle", "NSURL", "NSData",
        ],
        family: Some("c-family"),
        exclusive_patterns: &[],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"@interface\s+\w+", 5),
            wp!(r"@implementation\s+\w+", 5),
            wp!(r"@protocol\s+\w+", 4),
            wp!(r"@property\s*\(", 4),
            wp!(r"@synthesize\s+\w+", 4),
        ],
        hints: &[
            wp!(r"\[\w+\s+\w+[:\]]", 3),
            wp!(r"@selector\s*\(", 3),
            wp!(r"@autoreleasepool\s*\{", 3),
            wp!(r#"#import\s+[<"]"#, 3),
            wp!(r"\bNS\w{3,}\b", 2),
        ],
        rivals: &["c"],
        differentiators: &[
            wp!(r"@interface\s+\w+", 5),
            wp!(r"@implementation\s+\w+", 5),
            wp!(r"@protocol\s+\w+", 4),
            wp!(r"@property\s*\(", 4),
            wp!(r"\bNS\w{3,}\b", 3),
        ],
        disqualifiers: &[],
    }
}
