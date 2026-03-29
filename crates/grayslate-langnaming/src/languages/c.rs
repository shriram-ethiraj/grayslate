use std::collections::HashSet;

use super::{NamingDefinition, Extractor};
use crate::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "c",
        extension: "c",
        extract: Extractor::Custom(extract_c),
    }
}

/// C naming: regex-based (header guard, typedef, struct, enum, #define, function).
fn extract_c(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static HEADER_GUARD_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^#ifndef\s+([A-Z][A-Z0-9_]+_H(?:PP|XX)?)").unwrap()
    });
    static DEFINE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^#define\s+([A-Z][A-Z0-9_]{2,})(?:\s|$|\()").unwrap()
    });
    static TYPEDEF_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)typedef\s+(?:struct|union|enum)?\s*\{[^}]*\}\s*([A-Za-z_]\w+)\s*;")
            .unwrap()
    });
    static TYPEDEF_SIMPLE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^typedef\s+\w+\s+([A-Za-z_]\w+)\s*;").unwrap()
    });
    static ENUM_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^enum\s+([A-Za-z_]\w*)").unwrap());
    static STRUCT_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^struct\s+([A-Za-z_]\w*)").unwrap());
    static UNION_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^union\s+([A-Za-z_]\w*)").unwrap());
    static FUNC_DECL_RE: LazyLock<Regex> = LazyLock::new(|| {
        // Matches: [static] [inline] [const] <return_type> [*] name(
        // Also handles multi-word return types like "unsigned long" and pointer returns
        Regex::new(r"(?m)^(?:static\s+)?(?:inline\s+)?(?:extern\s+)?(?:const\s+)?(?:unsigned\s+|signed\s+|long\s+|short\s+)?(?:void|char|int|float|double|size_t|ssize_t|bool|_Bool|\w+_t)\s+\*?\s*([a-zA-Z_]\w*)\s*\(")
            .unwrap()
    });

    const NOISE: &[&str] = &[
        "main", "init", "test", "setup", "TRUE", "FALSE", "NULL", "EOF",
        "MAX", "MIN", "SIZE", "LEN",
    ];

    struct Sym { name: String, priority: u8 }
    let mut symbols: Vec<Sym> = Vec::new();

    // Header guard → derive a name from it
    if let Some(cap) = HEADER_GUARD_RE.captures(content) {
        let guard = &cap[1];
        let stem = guard
            .strip_suffix("_HPP").or_else(|| guard.strip_suffix("_HXX"))
            .or_else(|| guard.strip_suffix("_H"))
            .unwrap_or(guard);
        if !stem.is_empty() {
            return Some(stem.to_lowercase().replace('_', "-"));
        }
    }

    for cap in TYPEDEF_RE.captures_iter(content).take(3) {
        symbols.push(Sym { name: cap[1].to_string(), priority: 8 });
    }
    for cap in TYPEDEF_SIMPLE_RE.captures_iter(content).take(2) {
        symbols.push(Sym { name: cap[1].to_string(), priority: 8 });
    }
    for cap in ENUM_RE.captures_iter(content).take(2) {
        symbols.push(Sym { name: cap[1].to_string(), priority: 8 });
    }
    for cap in STRUCT_RE.captures_iter(content).take(2) {
        symbols.push(Sym { name: cap[1].to_string(), priority: 9 });
    }
    for cap in UNION_RE.captures_iter(content).take(2) {
        symbols.push(Sym { name: cap[1].to_string(), priority: 8 });
    }
    for cap in DEFINE_RE.captures_iter(content).take(3) {
        let name = &cap[1];
        if !NOISE.contains(&name) && !name.ends_with("_H") {
            symbols.push(Sym { name: name.to_string(), priority: 6 });
        }
    }
    for cap in FUNC_DECL_RE.captures_iter(content).take(3) {
        let name = cap[1].to_string();
        if !NOISE.contains(&name.as_str()) {
            symbols.push(Sym { name, priority: 7 });
        }
    }

    symbols.sort_by(|a, b| b.priority.cmp(&a.priority));

    let mut seen = HashSet::new();
    let mut tokens: Vec<String> = Vec::new();
    for sym in &symbols {
        if tokens.len() >= MAX_TOKENS { break; }
        if seen.insert(sym.name.clone()) {
            tokens.push(sym.name.clone());
        }
    }

    if tokens.is_empty() { None } else { Some(tokens.join("-")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_c(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn header_guard() {
        let src = "#ifndef MY_UTILS_H\n#define MY_UTILS_H\nvoid do_stuff();\n#endif";
        let n = name(src).unwrap();
        assert!(n.contains("my-utils"), "got: {n}");
    }

    #[test]
    fn typedef_struct() {
        let src = "typedef struct {\n    int x;\n    int y;\n} Point;\n";
        let n = name(src).unwrap();
        assert!(n.contains("point"), "got: {n}");
    }

    #[test]
    fn struct_and_function() {
        let src = "struct HashTable {\n    int size;\n    void **entries;\n};\nint hash_insert(struct HashTable *ht, const char *key, void *value) { return 0; }";
        let n = name(src).unwrap();
        assert!(n.contains("hash-table") || n.contains("hash-insert"), "got: {n}");
    }

    #[test]
    fn enum_declaration() {
        let src = "enum Color {\n    RED,\n    GREEN,\n    BLUE\n};";
        let n = name(src).unwrap();
        assert!(n.contains("color"), "got: {n}");
    }

    #[test]
    fn union_declaration() {
        let src = "union Data {\n    int i;\n    float f;\n    char *s;\n};";
        let n = name(src).unwrap();
        assert!(n.contains("data"), "got: {n}");
    }

    #[test]
    fn define_macro() {
        let src = "#define MAX_BUFFER_SIZE 1024\n#define DEFAULT_PORT 8080";
        let n = name(src).unwrap();
        assert!(n.contains("max-buffer-size"), "got: {n}");
    }

    #[test]
    fn function_declaration() {
        let src = "void process_data(int *buf, size_t len) {\n    // processing\n}";
        let n = name(src).unwrap();
        assert!(n.contains("process-data"), "got: {n}");
    }

    #[test]
    fn main_is_noise() {
        let src = "int main(int argc, char *argv[]) {\n    return 0;\n}";
        assert!(name(src).is_none(), "main should be filtered");
    }
}
