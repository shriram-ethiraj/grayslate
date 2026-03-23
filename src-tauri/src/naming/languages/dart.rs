use super::NamingDefinition;
use crate::naming::code::extract_with_regex;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "dart",
        extension: "dart",
        extract: extract_dart,
    }
}

fn extract_dart(content: &str) -> Option<String> {
    const PATTERNS: &[(&str, u8)] = &[
        (r"(?m)^(?:abstract\s+)?class\s+([A-Z]\w+)", 9),
        (r"(?m)^mixin\s+([A-Z]\w+)", 9),
        (r"(?m)^extension\s+([A-Z]\w+)", 8),
        (r"(?m)^enum\s+([A-Z]\w+)", 8),
        // Top-level functions (return type + name)
        (r"(?m)^(?:Future|Stream|void|int|double|String|bool|dynamic|List|Map|Set|\w+)\s+([a-zA-Z_]\w*)\s*\(", 7),
        // library/part of — fallback context
        (r"(?m)^library\s+([a-zA-Z_][\w.]+)\s*;", 5),
        (r"(?m)^part\s+of\s+([a-zA-Z_][\w.]+)\s*;", 5),
        // Top-level const declarations (common in Dart)
        (r"(?m)^const\s+\w+\s+([a-zA-Z_]\w+)\s*=", 4),
        // Top-level final declarations
        (r"(?m)^final\s+\w+\s+([a-zA-Z_]\w+)\s*=", 4),
    ];
    extract_with_regex(content, PATTERNS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_dart(src).and_then(|s| slugify(&s))
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
}
