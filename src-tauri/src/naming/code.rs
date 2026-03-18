use std::collections::HashSet;

use super::model::{CodeStyle, MAX_TOKENS};

// ---------------------------------------------------------------------------
// Code extractor (all programming languages via language-specific regex)
// ---------------------------------------------------------------------------

pub(super) fn extract_code(content: &str, style: CodeStyle) -> Option<String> {
    // Each style returns a list of regex patterns that capture a symbol name.
    let patterns: Vec<&str> = match style {
        CodeStyle::JsTs => vec![
            r"(?m)^(?:export\s+)?(?:default\s+)?class\s+([A-Z][a-zA-Z0-9_]*)",
            r"(?m)^(?:export\s+)?(?:async\s+)?function\s+([a-zA-Z_$][a-zA-Z0-9_$]*)",
            r"(?m)^(?:export\s+)?(?:const|let|var)\s+([A-Z][a-zA-Z0-9_$]*)\s*=",
            r"(?m)^(?:export\s+)?(?:interface|type)\s+([A-Z][a-zA-Z0-9_]*)",
        ],
        CodeStyle::Python => vec![
            r"(?m)^class\s+([A-Z][a-zA-Z0-9_]*)",
            r"(?m)^(?:async\s+)?def\s+([a-zA-Z_][a-zA-Z0-9_]*)",
        ],
        CodeStyle::Rust => vec![
            r"(?m)^(?:pub\s+)?(?:struct|enum|trait|impl)\s+([A-Z][a-zA-Z0-9_]*)",
            r"(?m)^(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)",
            r"(?m)^(?:pub\s+)?mod\s+([a-zA-Z_][a-zA-Z0-9_]*)",
        ],
        CodeStyle::JavaLike => vec![
            r"(?m)(?:public|private|protected)?\s*(?:abstract\s+)?class\s+([A-Z][a-zA-Z0-9_]*)",
            r"(?m)(?:public|private|protected)?\s*interface\s+([A-Z][a-zA-Z0-9_]*)",
            r"(?m)(?:public|private|protected)[\s\w<>\[\]]+\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(",
        ],
        CodeStyle::Go => vec![
            r"(?m)^type\s+([A-Z][a-zA-Z0-9_]*)\s+(?:struct|interface)",
            r"(?m)^func\s+(?:\([^)]+\)\s+)?([A-Z][a-zA-Z0-9_]*)\s*\(",
        ],
        CodeStyle::CFamily => vec![
            r"(?m)^(?:class|struct)\s+([A-Z][a-zA-Z0-9_]*)",
            r"(?m)^(?:\w+[\s\*]+)+([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^)]*\)\s*\{",
        ],
        CodeStyle::CSharp => vec![
            r"(?m)(?:public|private|protected|internal)?\s*(?:partial\s+)?(?:class|interface|struct|enum)\s+([A-Z][a-zA-Z0-9_]*)",
            r"(?m)(?:public|private|protected|internal)[\s\w<>\[\]?]+\s+([A-Z][a-zA-Z0-9_]*)\s*\(",
        ],
        CodeStyle::Swift => vec![
            r"(?m)^(?:public\s+|private\s+|internal\s+)?(?:class|struct|enum|protocol)\s+([A-Z][a-zA-Z0-9_]*)",
            r"(?m)^(?:public\s+|private\s+|internal\s+)?func\s+([a-zA-Z_][a-zA-Z0-9_]*)",
        ],
        CodeStyle::Ruby => vec![
            r"(?m)^class\s+([A-Z][a-zA-Z0-9_:]*)",
            r"(?m)^def\s+([a-zA-Z_][a-zA-Z0-9_?!]*)",
        ],
        CodeStyle::Php => vec![
            r"(?m)^(?:abstract\s+)?class\s+([A-Z][a-zA-Z0-9_]*)",
            r"(?m)^(?:public|private|protected)?\s*(?:static\s+)?function\s+([a-zA-Z_][a-zA-Z0-9_]*)",
        ],
        CodeStyle::Dart => vec![
            r"(?m)^(?:abstract\s+)?class\s+([A-Z][a-zA-Z0-9_]*)",
            r"(?m)^(?:\w+\s+)+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(",
        ],
        CodeStyle::Shell => vec![
            r"(?m)^([a-zA-Z_][a-zA-Z0-9_]*)\s*\(\s*\)",
            r"(?m)^function\s+([a-zA-Z_][a-zA-Z0-9_]*)",
        ],
    };

    let mut seen: HashSet<String> = HashSet::new();
    let mut tokens: Vec<String> = Vec::new();

    for pattern in &patterns {
        if tokens.len() >= MAX_TOKENS {
            break;
        }
        if let Ok(re) = regex::Regex::new(pattern) {
            for cap in re.captures_iter(content).take(3) {
                if tokens.len() >= MAX_TOKENS {
                    break;
                }
                if let Some(m) = cap.get(1) {
                    let name = m.as_str().to_string();
                    if !name.is_empty() && seen.insert(name.clone()) {
                        tokens.push(name);
                    }
                }
            }
        }
    }

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join("-"))
    }
}
