use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition()-> LanguageDefinition {
    LanguageDefinition {
        name: "swift",
        extensions: &[".swift"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
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
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"(?m)^\s*import\s+(Foundation|UIKit|SwiftUI|Combine|CoreData|MapKit|AVFoundation|CoreGraphics|AppKit)\b", 5),
            wp!(r"\bguard\s+let\s+\w+", 5),
            wp!(r"\b@(IBOutlet|IBAction|objc|escaping|Published|State|Binding|Observable|MainActor)\b", 5),
            wp!(r"\bweak\s+var\s+", 4),
            wp!(r"\bif\s+let\s+\w+\s*=", 4),
            // Swift actor concurrency
            wp!(r"(?m)^\s*(public\s+|internal\s+|private\s+)?actor\s+\w+", 5),
            // Task { } — Swift structured concurrency
            wp!(r"\bTask\s*\{", 4),
            // @main struct/class — SwiftUI / Swift entry point
            wp!(r"(?m)^\s*@main\s+(struct|class)\s+\w+", 5),
            // struct X: View — SwiftUI view
            wp!(r"\bstruct\s+\w+\s*:\s*(some\s+)?View\b", 5),
            // async throws — Swift concurrency error handling
            wp!(r"\basync\s+throws\b", 4),
        ],
        hints: &[
            wp!(r"(?m)^\s*func\s+\w+\s*\(", 3),
            wp!(r"(?m)\bextension\s+\w+", 3),
            wp!(r"\bguard\s+\w+", 3),
            wp!(r"\boptional\s+func\b", 3),
            // Optional chaining: foo?.bar
            wp!(r"\?\.\w+", 2),
            // try? / try! — Swift error handling
            wp!(r"\btry[?!]\s+", 3),
            // some — opaque return types (Swift 5.1)
            wp!(r"\bsome\s+\w+", 2),
            // deinit — Swift destructor
            wp!(r"(?m)^\s*deinit\s*\{", 3),
            // let/var with type annotation: let x: Type
            wp!(r"\b(let|var)\s+\w+\s*:\s*\w+", 2),
            // protocol conformance: struct X: Protocol
            wp!(r"(?m)^\s*(struct|class|enum)\s+\w+\s*:\s*\w+", 2),
        ],
        disqualifiers: &[
            // Rust macro syntax — not Swift
            wp!(r"\bprintln!\s*\(", -5),
        ],
    }
}
