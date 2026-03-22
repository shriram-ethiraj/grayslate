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
        (r"(?m)^(?:Future|Stream|void|int|double|String|bool|dynamic|\w+)\s+([a-zA-Z_]\w*)\s*\(", 7),
    ];
    extract_with_regex(content, PATTERNS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dart_class() {
        let src = "class UserRepository {\n  Future<User> findById(int id) async {}\n}";
        assert!(extract_dart(src).unwrap().contains("UserRepository"));
    }

    #[test]
    fn dart_mixin() {
        let src = "mixin Draggable on Widget {\n  void drag() {}\n}";
        assert!(extract_dart(src).unwrap().contains("Draggable"));
    }
}
