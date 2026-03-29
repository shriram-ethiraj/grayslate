use super::{NamingDefinition, Extractor};
use crate::code::SymbolPattern;

/// Swift-specific noise names (lifecycle methods, protocol requirements).
static NOISE: &[&str] = &[
    "body", "viewDidLoad", "viewWillAppear", "viewDidAppear",
    "encode", "decode", "hash", "description",
];

/// Swift symbol patterns for declarative extraction.
///
/// Priority order:
///   10 – @main decorated types
///    9 – types: class, struct, enum, protocol, actor
///    7 – func declarations
///    6 – extensions
static PATTERNS: &[SymbolPattern] = &[
    // @main types (highest priority)
    SymbolPattern { regex: r"(?m)@main\s+(?:(?:public|internal|private)\s+)?(?:struct|class|enum)\s+([A-Z][a-zA-Z0-9_]*)", priority: 10, capture_group: 1 },
    // Types
    SymbolPattern { regex: r"(?m)^[ \t]*(?:(?:public|private|internal|open|final)\s+)?(?:class|struct|enum|protocol|actor)\s+([A-Z][a-zA-Z0-9_]*)", priority: 9, capture_group: 1 },
    // Functions
    SymbolPattern { regex: r"(?m)^[ \t]*(?:(?:public|private|internal|open|override|static|class)\s+)*func\s+([a-zA-Z_][a-zA-Z0-9_]*)", priority: 7, capture_group: 1 },
    // Extensions
    SymbolPattern { regex: r"(?m)^extension\s+([A-Z][a-zA-Z0-9_]*)", priority: 6, capture_group: 1 },
];

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "swift",
        extension: "swift",
        extract: Extractor::Patterns { symbols: PATTERNS, noise: NOISE },
    }
}

#[cfg(test)]
mod tests {
    use crate::code::extract_from_patterns;
    use crate::shared::slugify;
    use crate::suggest_stem;
    use super::{PATTERNS, NOISE};

    fn name(src: &str) -> Option<String> {
        extract_from_patterns(src, PATTERNS, NOISE).and_then(|s| slugify(&s))
    }

    #[test]
    fn class_and_protocol() {
        let src = "protocol Cacheable {\n    func cache()\n}\n\nclass ImageCache: Cacheable {\n    func cache() { }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("cacheable") || n.contains("image-cache"), "got: {n}");
    }

    #[test]
    fn actor_type() {
        let src = "actor TemperatureLogger {\n    var measurements: [Int]\n}";
        let n = name(src).unwrap();
        assert!(n.contains("temperature-logger"), "got: {n}");
    }

    #[test]
    fn main_app() {
        let src = "@main\nstruct MyApp {\n    var body: some Scene { }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("my-app"), "got: {n}");
    }

    #[test]
    fn noise_filtered() {
        let src = "class ViewController: UIViewController {\n    override func viewDidLoad() { }\n    func configureUI() { }\n}";
        let n = name(src).unwrap();
        assert!(!n.contains("view-did-load"), "noise filtered: {n}");
        assert!(n.contains("view-controller"), "type name kept: {n}");
    }

    #[test]
    fn swift_integration() {
        let stem = suggest_stem("struct ContentView: View {\n    var body: some View { Text(\"Hello\") }\n}", "swift").unwrap();
        assert!(stem.contains("content-view"), "integration: {stem}");
    }

    #[test]
    fn extension_type() {
        let src = "extension DateFormatter {\n    static var iso8601: DateFormatter {\n        let fmt = DateFormatter()\n        return fmt\n    }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("date-formatter"), "extension type: {n}");
    }

    #[test]
    fn struct_and_enum() {
        let src = "struct Point {\n    let x: Double\n    let y: Double\n}\n\nenum Direction {\n    case north, south, east, west\n}";
        let n = name(src).unwrap();
        assert!(n.contains("point"), "struct wins: {n}");
    }
}

