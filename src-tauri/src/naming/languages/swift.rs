use std::collections::HashSet;

use super::NamingDefinition;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "swift",
        extension: "swift",
        extract: extract_swift,
    }
}

/// Swift regex extraction with improved coverage.
///
/// Priority order:
///   1. `import` framework — P5 (context only)
///   2. `protocol` / `class` / `struct` / `enum` / `actor` — P9
///   3. `func` declarations — P7
///   4. `@main` decorated types — P10
fn extract_swift(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static MAIN_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)@main\s+(?:(?:public|internal|private)\s+)?(?:struct|class|enum)\s+([A-Z][a-zA-Z0-9_]*)").unwrap()
    });
    static TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?m)^[ \t]*(?:(?:public|private|internal|open|final)\s+)?(?:class|struct|enum|protocol|actor)\s+([A-Z][a-zA-Z0-9_]*)",
        )
        .unwrap()
    });
    static FUNC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^[ \t]*(?:(?:public|private|internal|open|override|static|class)\s+)*func\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap()
    });
    static EXT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^extension\s+([A-Z][a-zA-Z0-9_]*)").unwrap()
    });

    const NOISE: &[&str] = &[
        "main", "init", "setup", "run", "start", "body", "app",
        "viewDidLoad", "viewWillAppear", "viewDidAppear",
        "encode", "decode", "hash", "description",
    ];

    struct Symbol { name: String, priority: u8 }
    let mut symbols: Vec<Symbol> = Vec::new();

    // @main types (P10)
    for cap in MAIN_RE.captures_iter(content).take(1) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 10 });
    }

    // Types (P9)
    for cap in TYPE_RE.captures_iter(content).take(4) {
        let name = &cap[1];
        if !NOISE.contains(&name) {
            symbols.push(Symbol { name: name.to_string(), priority: 9 });
        }
    }

    // Extensions (P6)
    for cap in EXT_RE.captures_iter(content).take(2) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 6 });
    }

    // Functions (P7)
    for cap in FUNC_RE.captures_iter(content).take(4) {
        let name = &cap[1];
        if !NOISE.contains(&name) {
            symbols.push(Symbol { name: name.to_string(), priority: 7 });
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
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_swift(src).and_then(|s| slugify(&s))
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
}

