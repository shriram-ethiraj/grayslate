use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition()-> LanguageDefinition {
    LanguageDefinition {
        name: "objectivec",
        // `.mm` is Objective-C++ and must not be claimed here: extension
        // detection is deterministic and uses the first matching definition.
        extensions: &[".m"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
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
            "UIView", "UIViewController", "UILabel", "UIButton", "UITableView",
        ],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"@interface\s+\w+", 5),
            wp!(r"@implementation\s+\w+", 5),
            wp!(r"@protocol\s+\w+", 5),
            wp!(r"@property\s*\(", 4),
            wp!(r"@synthesize\s+\w+", 4),
            // #import is ObjC-specific (C uses #include)
            wp!(r#"#import\s+[<"]"#, 5),
            // @end terminates every ObjC class/protocol/category
            wp!(r"(?m)^@end\b", 4),
            // Message passing: [object method:arg]
            wp!(r"\[\w+\s+\w+:", 4),
            // @autoreleasepool
            wp!(r"@autoreleasepool\s*\{", 4),
            // NSLog — ObjC logging
            wp!(r"\bNSLog\s*\(", 4),
        ],
        hints: &[
            wp!(r"\[\w+\s+\w+[:\]]", 3),
            wp!(r"@selector\s*\(", 3),
            // NS* Foundation classes (broad)
            wp!(r"\bNS\w{3,}\b", 2),
            // YES/NO boolean literals
            wp!(r"\b(YES|NO)\b", 2),
            // @"string literal"
            wp!(r#"@""#, 2),
            // ObjC method declaration: - (void)methodName
            wp!(r"(?m)^[-+]\s*\([^)]+\)\s*\w+", 3),
            // Property attributes: (nonatomic, strong)
            wp!(r"\(nonatomic\b", 2),
            // @dynamic
            wp!(r"@dynamic\s+\w+", 2),
            // UI* classes (UIKit)
            wp!(r"\bUI\w{4,}\b", 2),
        ],
        disqualifiers: &[
            // C++ exclusive syntax — if present, this is ObjC++ not ObjC
            wp!(r"(?m)\bnamespace\s+\w+", -4),
            wp!(r"\bstd::\w+", -3),
            wp!(r"(?m)\btemplate\s*<", -4),
        ],
    }
}
