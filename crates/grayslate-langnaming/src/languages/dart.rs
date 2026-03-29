use super::{NamingDefinition, Extractor};
use crate::code::SymbolPattern;

/// Dart symbol patterns for declarative extraction.
static PATTERNS: &[SymbolPattern] = &[
    SymbolPattern { regex: r"(?m)^(?:abstract\s+)?class\s+([A-Z]\w+)", priority: 9, capture_group: 1 },
    SymbolPattern { regex: r"(?m)^mixin\s+([A-Z]\w+)", priority: 9, capture_group: 1 },
    SymbolPattern { regex: r"(?m)^extension\s+([A-Z]\w+)", priority: 8, capture_group: 1 },
    SymbolPattern { regex: r"(?m)^enum\s+([A-Z]\w+)", priority: 8, capture_group: 1 },
    // Top-level functions (return type + name)
    SymbolPattern { regex: r"(?m)^(?:Future|Stream|void|int|double|String|bool|dynamic|List|Map|Set|\w+)\s+([a-zA-Z_]\w*)\s*\(", priority: 7, capture_group: 1 },
    // library/part of — fallback context
    SymbolPattern { regex: r"(?m)^library\s+([a-zA-Z_][\w.]+)\s*;", priority: 5, capture_group: 1 },
    SymbolPattern { regex: r"(?m)^part\s+of\s+([a-zA-Z_][\w.]+)\s*;", priority: 5, capture_group: 1 },
    // Top-level const/final declarations
    SymbolPattern { regex: r"(?m)^const\s+\w+\s+([a-zA-Z_]\w+)\s*=", priority: 4, capture_group: 1 },
    SymbolPattern { regex: r"(?m)^final\s+\w+\s+([a-zA-Z_]\w+)\s*=", priority: 4, capture_group: 1 },
];

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "dart",
        extension: "dart",
        extract: Extractor::Patterns { symbols: PATTERNS, noise: &[] },
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;
    use crate::shared::slugify;
    use crate::code::extract_from_patterns;
    use super::PATTERNS;

    fn name(src: &str) -> Option<String> {
        extract_from_patterns(src, PATTERNS, &[]).and_then(|s| slugify(&s))
    }

    #[test]
    fn dart_class() {
        let src = "class UserRepository {\n  Future<User> findById(int id) async {}\n}";
        let n = name(src).unwrap();
        assert!(n.contains("user-repository"), "got: {n}");
    }

    #[test]
    fn dart_mixin() {
        let src = "mixin Draggable on Widget {\n  void drag() {}\n}";
        let n = name(src).unwrap();
        assert!(n.contains("draggable"), "got: {n}");
    }

    #[test]
    fn dart_const_file() {
        let src = "const double galleryHeaderHeight = 64;\nconst double desktopDisplay1FontDelta = 16;\n";
        let n = name(src).unwrap();
        assert!(n.contains("gallery-header-height"), "const extraction: {n}");
    }

    #[test]
    fn dart_library() {
        let src = "library my_widgets;\n\nclass Button extends StatelessWidget {}";
        let n = name(src).unwrap();
        // Class (P9) beats library (P5)
        assert!(n.contains("button"), "class beats library: {n}");
    }

    #[test]
    fn dart_enum() {
        let src = "enum ThemeMode {\n  light,\n  dark,\n  system,\n}";
        let n = name(src).unwrap();
        assert!(n.contains("theme-mode"), "got: {n}");
    }

    #[test]
    fn dart_integration_suggest_stem() {
        let stem = suggest_stem("class HttpClient {\n  Future<Response> get(String url) async {}\n}", "dart").unwrap();
        assert!(stem.contains("http-client"), "integration test: {stem}");
    }

    #[test]
    fn dart_abstract_class() {
        let src = "abstract class Validator<T> {\n  bool validate(T value);\n  String get errorMessage;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("validator"), "abstract class: {n}");
    }

    #[test]
    fn dart_extension() {
        let src = "extension NumberFormatting on num {\n  String toPercentage() => '${(this * 100).toStringAsFixed(1)}%';\n}";
        let n = name(src).unwrap();
        assert!(n.contains("number-formatting"), "extension: {n}");
    }
}
