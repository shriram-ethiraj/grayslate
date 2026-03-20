/// Tree-sitter validation for ambiguous detection results.
///
/// When Phase 4 heuristic scoring produces ambiguous results (top two
/// candidates within a small margin), we use tree-sitter to validate
/// by attempting to parse the content with each candidate's grammar.
///
/// The grammar with fewer error nodes wins.
use tree_sitter::Parser;

/// Maximum bytes to feed tree-sitter for validation (keep it fast).
const MAX_VALIDATION_BYTES: usize = 5_000;

/// Languages we can validate via tree-sitter.
/// Maps language ID → tree-sitter Language function.
fn get_ts_language(lang: &str) -> Option<tree_sitter::Language> {
    match lang {
        "javascript" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "c" => Some(tree_sitter_c::LANGUAGE.into()),
        "cpp" => Some(tree_sitter_cpp::LANGUAGE.into()),
        _ => None,
    }
}

/// Validate a candidate language by parsing with tree-sitter.
///
/// Returns an error ratio: 0.0 = perfect parse, 1.0 = all errors.
/// Returns `None` if the language has no tree-sitter grammar.
fn validation_error_ratio(content: &str, lang: &str) -> Option<f64> {
    let ts_lang = get_ts_language(lang)?;

    let bounded = if content.len() > MAX_VALIDATION_BYTES {
        &content[..MAX_VALIDATION_BYTES]
    } else {
        content
    };

    let mut parser = Parser::new();
    parser.set_language(&ts_lang).ok()?;

    let tree = parser.parse(bounded, None)?;
    let root = tree.root_node();

    let total_nodes = count_nodes(&root);
    if total_nodes == 0 {
        return Some(1.0);
    }

    let error_nodes = count_error_nodes(&root);
    Some(error_nodes as f64 / total_nodes as f64)
}

fn count_nodes(node: &tree_sitter::Node) -> usize {
    let mut count = 1;
    let child_count = node.child_count();
    for i in 0..child_count {
        if let Some(child) = node.child(i) {
            count += count_nodes(&child);
        }
    }
    count
}

fn count_error_nodes(node: &tree_sitter::Node) -> usize {
    let mut count = if node.is_error() || node.is_missing() {
        1
    } else {
        0
    };
    let child_count = node.child_count();
    for i in 0..child_count {
        if let Some(child) = node.child(i) {
            count += count_error_nodes(&child);
        }
    }
    count
}

/// Given two candidate languages, use tree-sitter to pick the better one.
///
/// Returns `Some(language)` if tree-sitter can differentiate, `None` if
/// neither language has a grammar or both parse equally well/poorly.
pub fn validate_candidates(content: &str, candidate_a: &str, candidate_b: &str) -> Option<&'static str> {
    let ratio_a = validation_error_ratio(content, candidate_a);
    let ratio_b = validation_error_ratio(content, candidate_b);

    match (ratio_a, ratio_b) {
        (Some(a), Some(b)) => {
            // Require a meaningful difference (>5% error ratio gap)
            if (a - b).abs() < 0.05 {
                None
            } else if a < b {
                // candidate_a parses better — return its static str
                lang_to_static(candidate_a)
            } else {
                lang_to_static(candidate_b)
            }
        }
        (Some(_), None) => lang_to_static(candidate_a),
        (None, Some(_)) => lang_to_static(candidate_b),
        (None, None) => None,
    }
}

/// Convert a language string to a &'static str (matches our known IDs).
fn lang_to_static(lang: &str) -> Option<&'static str> {
    match lang {
        "javascript" => Some("javascript"),
        "typescript" => Some("typescript"),
        "python" => Some("python"),
        "rust" => Some("rust"),
        "java" => Some("java"),
        "go" => Some("go"),
        "c" => Some("c"),
        "cpp" => Some("cpp"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_parses_cleanly() {
        let content = "def hello():\n    print('hello')\n";
        let ratio = validation_error_ratio(content, "python");
        assert!(ratio.is_some());
        assert!(ratio.unwrap() < 0.1, "Python should parse cleanly");
    }

    #[test]
    fn rust_parses_cleanly() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let ratio = validation_error_ratio(content, "rust");
        assert!(ratio.is_some());
        assert!(ratio.unwrap() < 0.1, "Rust should parse cleanly");
    }

    #[test]
    fn unknown_language_returns_none() {
        assert!(validation_error_ratio("test", "clojure").is_none());
    }

    #[test]
    fn typescript_vs_javascript() {
        let ts_content = r#"
interface User {
    name: string;
    age: number;
}

const getUser = (id: number): User => {
    return { name: "Alice", age: 30 };
};
"#;
        // TypeScript should parse better than JavaScript for TS content
        let ts_ratio = validation_error_ratio(ts_content, "typescript").unwrap();
        let js_ratio = validation_error_ratio(ts_content, "javascript").unwrap();
        assert!(
            ts_ratio <= js_ratio,
            "TS should parse TS content at least as well: ts={ts_ratio}, js={js_ratio}"
        );
    }
}
