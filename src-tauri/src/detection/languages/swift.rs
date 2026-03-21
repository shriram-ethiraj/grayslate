use super::{wp, LanguageDefinition};

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "swift",
        extensions: &[".swift"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"(?m)^\s*func\s+\w+\s*\(", 3),
            wp!(r"(?m)^\s*import\s+(Foundation|UIKit|SwiftUI|Combine)\b", 5),
            wp!(r"\bguard\s+let\s+\w+", 5),
            wp!(r"\bguard\s+\w+", 3),
            wp!(r"(?m)\b(struct|class|enum|protocol)\s+\w+\s*[:\{]", 2),
            wp!(r"\bweak\s+var\s+", 4),
            wp!(r"\blet\s+\w+\s*:\s*\w+", 2),
            wp!(r"\bvar\s+\w+\s*:\s*\w+", 2),
            wp!(r"\bif\s+let\s+\w+\s*=", 4),
            wp!(r"\bswitch\s+\w+\s*\{", 1),
            wp!(r#"\bprint\s*\(""#, 1),
            wp!(r"\b@(IBOutlet|IBAction|objc|escaping|Published|State|Binding)\b", 5),
            wp!(r"(?m)\bextension\s+\w+", 3),
            wp!(r"\boptional\s+func\b", 3),
            wp!(r"\b(String|Int|Double|Bool|Array|Dictionary)<?\b", 1),
        ],
        anti_patterns: &[
            wp!(r"\bprintln!\s*\(", -5),
        ],
        uses_hash_comments: false,
        keywords: &[
            "func", "guard", "let", "var", "protocol", "extension",
            "struct", "enum", "defer", "where", "repeat", "rethrows",
            "fallthrough", "associatedtype", "typealias", "operator",
            "inout", "indirect", "convenience", "required", "optional",
            "fileprivate", "open", "internal", "willset", "didset",
            "subscript", "deinit", "init", "weak", "unowned", "lazy",
        ],
        builtins: &[
            "print", "string", "array", "dictionary", "set",
            "int", "double", "float", "bool", "character",
            "uiviewcontroller", "uiview", "uibutton", "uilabel",
            "nsobject", "cgfloat", "cgrect", "cgpoint", "cgsize",
            "dispatchqueue", "urlsession", "codable", "encodable",
            "decodable", "identifiable", "hashable", "equatable",
        ],
        illegal: None,
        extends: None,
    }
}
