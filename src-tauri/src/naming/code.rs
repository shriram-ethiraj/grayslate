use std::collections::HashSet;

use super::model::{CodeStyle, MAX_TOKENS};
use tree_sitter::Parser;
use tree_sitter_language::LanguageFn;

// ---------------------------------------------------------------------------
// Code extractor — tree-sitter AST with regex fallback
// ---------------------------------------------------------------------------

/// Noise symbol names that are too generic to be useful in a filename.
const NOISE_NAMES: &[&str] = &[
    "main", "init", "setup", "run", "start", "new", "default", "handle",
    "index", "app", "mod", "test", "self", "this", "cls",
];

fn is_noise_name(name: &str) -> bool {
    NOISE_NAMES.contains(&name)
}

/// A collected symbol with its naming priority.
struct Symbol {
    name: String,
    priority: u8,
}

pub(super) fn extract_code(content: &str, style: CodeStyle) -> Option<String> {
    try_tree_sitter(content, style)
        .or_else(|| extract_code_regex(content, style))
}

// ---------------------------------------------------------------------------
// tree-sitter AST extraction
// ---------------------------------------------------------------------------

fn try_tree_sitter(content: &str, style: CodeStyle) -> Option<String> {
    let language_fn: LanguageFn = match style {
        CodeStyle::JsTs => tree_sitter_javascript::LANGUAGE,
        CodeStyle::Python => tree_sitter_python::LANGUAGE,
        CodeStyle::Rust => tree_sitter_rust::LANGUAGE,
        CodeStyle::JavaLike => tree_sitter_java::LANGUAGE,
        CodeStyle::Go => tree_sitter_go::LANGUAGE,
        CodeStyle::CFamily => pick_c_or_cpp_grammar(content),
        // No tree-sitter grammars for these — fall back to regex.
        CodeStyle::CSharp | CodeStyle::Swift | CodeStyle::Ruby
        | CodeStyle::Php | CodeStyle::Dart | CodeStyle::Shell => return None,
    };

    let mut parser = Parser::new();
    parser.set_language(&language_fn.into()).ok()?;
    let tree = parser.parse(content, None)?;
    let root = tree.root_node();
    let src = content.as_bytes();

    let mut symbols: Vec<Symbol> = Vec::new();
    match style {
        CodeStyle::Python => collect_python(&root, src, &mut symbols),
        CodeStyle::JsTs => collect_js_ts(&root, src, &mut symbols, content),
        CodeStyle::Rust => collect_rust(&root, src, &mut symbols),
        CodeStyle::JavaLike => collect_java(&root, src, &mut symbols),
        CodeStyle::Go => collect_go(&root, src, &mut symbols),
        CodeStyle::CFamily => collect_c_cpp(&root, src, &mut symbols),
        _ => {}
    }

    // Sort by priority descending, then take unique non-noise names.
    symbols.sort_by(|a, b| b.priority.cmp(&a.priority));

    let mut seen = HashSet::new();
    let mut tokens: Vec<String> = Vec::new();
    for sym in &symbols {
        if tokens.len() >= MAX_TOKENS {
            break;
        }
        if !is_noise_name(&sym.name) && seen.insert(sym.name.clone()) {
            tokens.push(sym.name.clone());
        }
    }

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join("-"))
    }
}

/// Heuristic to pick C++ grammar when content looks like C++.
fn pick_c_or_cpp_grammar(content: &str) -> LanguageFn {
    let sample = if content.len() > 2000 { &content[..2000] } else { content };
    if sample.contains("class ")
        || sample.contains("namespace ")
        || sample.contains("template<")
        || sample.contains("template <")
        || sample.contains("::")
        || sample.contains("std::")
        || sample.contains("#include <iostream>")
        || sample.contains("#include <vector>")
        || sample.contains("#include <string>")
    {
        tree_sitter_cpp::LANGUAGE
    } else {
        tree_sitter_c::LANGUAGE
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract text of a named field from a node.
fn field_text<'a>(node: &tree_sitter::Node, field: &str, src: &'a [u8]) -> Option<&'a str> {
    node.child_by_field_name(field)?
        .utf8_text(src)
        .ok()
}

/// Check whether a Rust node has a `pub` visibility modifier as a direct child.
fn has_pub_child(node: &tree_sitter::Node, src: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            if let Ok(text) = child.utf8_text(src) {
                if text.starts_with("pub") {
                    return true;
                }
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

fn collect_python(root: &tree_sitter::Node, src: &[u8], symbols: &mut Vec<Symbol>) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "class_definition" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 9 });
                }
            }
            "function_definition" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 7 });
                }
            }
            "decorated_definition" => {
                // Unwrap the inner definition.
                if let Some(inner) = child.child_by_field_name("definition") {
                    match inner.kind() {
                        "class_definition" => {
                            if let Some(name) = field_text(&inner, "name", src) {
                                symbols.push(Symbol { name: name.to_string(), priority: 9 });
                            }
                        }
                        "function_definition" => {
                            if let Some(name) = field_text(&inner, "name", src) {
                                symbols.push(Symbol { name: name.to_string(), priority: 7 });
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// JavaScript / TypeScript
// ---------------------------------------------------------------------------

fn collect_js_ts(
    root: &tree_sitter::Node,
    src: &[u8],
    symbols: &mut Vec<Symbol>,
    content: &str,
) {
    // If the JS grammar can't parse TypeScript-specific syntax, try the TS
    // grammar. We detect TS by checking for common TS-only patterns.
    let is_typescript = content.contains(": string")
        || content.contains(": number")
        || content.contains(": boolean")
        || content.contains("interface ")
        || content.contains("type ");

    if is_typescript {
        if let Some(result) = try_typescript_grammar(content) {
            *symbols = result;
            return;
        }
    }

    collect_js_nodes(root, src, symbols, false);
}

fn try_typescript_grammar(content: &str) -> Option<Vec<Symbol>> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .ok()?;
    let tree = parser.parse(content, None)?;
    let root = tree.root_node();
    let src = content.as_bytes();

    let mut symbols = Vec::new();
    collect_js_nodes(&root, src, &mut symbols, true);
    if symbols.is_empty() {
        None
    } else {
        Some(symbols)
    }
}

/// Shared JS/TS node collection. `is_ts` enables TS-specific node types.
fn collect_js_nodes(
    root: &tree_sitter::Node,
    src: &[u8],
    symbols: &mut Vec<Symbol>,
    is_ts: bool,
) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "class_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 9 });
                }
            }
            "function_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 7 });
                }
            }
            "interface_declaration" if is_ts => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 9 });
                }
            }
            "type_alias_declaration" if is_ts => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 8 });
                }
            }
            "export_statement" => {
                // Unwrap the exported declaration for higher priority.
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    match inner.kind() {
                        "class_declaration" => {
                            if let Some(name) = field_text(&inner, "name", src) {
                                symbols.push(Symbol { name: name.to_string(), priority: 9 });
                            }
                        }
                        "function_declaration" => {
                            if let Some(name) = field_text(&inner, "name", src) {
                                symbols.push(Symbol { name: name.to_string(), priority: 8 });
                            }
                        }
                        "interface_declaration" if is_ts => {
                            if let Some(name) = field_text(&inner, "name", src) {
                                symbols.push(Symbol { name: name.to_string(), priority: 9 });
                            }
                        }
                        "type_alias_declaration" if is_ts => {
                            if let Some(name) = field_text(&inner, "name", src) {
                                symbols.push(Symbol { name: name.to_string(), priority: 8 });
                            }
                        }
                        "lexical_declaration" => {
                            // Exported consts: accept any name (not just uppercase).
                            collect_lexical_decl(&inner, src, symbols, 8, false);
                        }
                        // export { Foo, Bar as Baz } — named re-exports.
                        "export_clause" => {
                            let mut spec_cursor = inner.walk();
                            for spec in inner.children(&mut spec_cursor) {
                                if spec.kind() == "export_specifier" {
                                    // Prefer the public alias over the original name.
                                    let name = field_text(&spec, "alias", src)
                                        .or_else(|| field_text(&spec, "name", src));
                                    if let Some(n) = name {
                                        if !is_noise_name(&n.to_lowercase()) {
                                            symbols.push(Symbol {
                                                name: n.to_string(),
                                                priority: 7,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        // export default someCall(...) — extract callee name.
                        "call_expression" => {
                            if let Some(func) = field_text(&inner, "function", src) {
                                if !is_noise_name(&func.to_lowercase()) {
                                    symbols.push(Symbol { name: func.to_string(), priority: 5 });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "lexical_declaration" => {
                // Top-level non-exported: only grab PascalCase (likely components/ctors).
                collect_lexical_decl(&child, src, symbols, 6, true);
            }
            _ => {}
        }
    }
}

/// Extract variable names from const/let/var declarations.
/// When `require_uppercase` is true, only names starting with an uppercase
/// letter are included (component/constructor heuristic for non-exported consts).
/// For exported declarations pass `false` to capture any meaningful name.
fn collect_lexical_decl(
    node: &tree_sitter::Node,
    src: &[u8],
    symbols: &mut Vec<Symbol>,
    priority: u8,
    require_uppercase: bool,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            if let Some(name) = field_text(&child, "name", src) {
                let significant = if require_uppercase {
                    name.starts_with(|c: char| c.is_uppercase())
                } else {
                    // Any name that starts with a letter/underscore is valid.
                    name.starts_with(|c: char| c.is_alphabetic() || c == '_')
                };
                if significant {
                    symbols.push(Symbol { name: name.to_string(), priority });
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

fn collect_rust(root: &tree_sitter::Node, src: &[u8], symbols: &mut Vec<Symbol>) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "mod_item" => {
                if let Some(name) = field_text(&child, "name", src) {
                    let pri = if has_pub_child(&child, src) { 10 } else { 8 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            "struct_item" => {
                if let Some(name) = field_text(&child, "name", src) {
                    let pri = if has_pub_child(&child, src) { 9 } else { 6 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            "enum_item" => {
                if let Some(name) = field_text(&child, "name", src) {
                    let pri = if has_pub_child(&child, src) { 9 } else { 6 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            "trait_item" => {
                if let Some(name) = field_text(&child, "name", src) {
                    let pri = if has_pub_child(&child, src) { 9 } else { 6 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            "impl_item" => {
                // Extract the type being implemented.
                if let Some(type_node) = child.child_by_field_name("type") {
                    if let Ok(name) = type_node.utf8_text(src) {
                        // Skip generic params: `Config<T>` → `Config`
                        let clean = name.split('<').next().unwrap_or(name).trim();
                        if !clean.is_empty() {
                            symbols.push(Symbol { name: clean.to_string(), priority: 5 });
                        }
                    }
                }
            }
            "function_item" => {
                if let Some(name) = field_text(&child, "name", src) {
                    let pri = if has_pub_child(&child, src) { 7 } else { 5 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Java
// ---------------------------------------------------------------------------

fn collect_java(root: &tree_sitter::Node, src: &[u8], symbols: &mut Vec<Symbol>) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "package_declaration" => {
                // Extract last segment of package name: `com.example.auth` → `auth`
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(text) = name_node.utf8_text(src) {
                        if let Some(last) = text.rsplit('.').next() {
                            if !last.is_empty() {
                                symbols.push(Symbol { name: last.to_string(), priority: 10 });
                            }
                        }
                    }
                }
            }
            "class_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 9 });
                }
            }
            "interface_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 9 });
                }
            }
            // Methods inside classes are NOT collected — they're not top-level.
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

fn collect_go(root: &tree_sitter::Node, src: &[u8], symbols: &mut Vec<Symbol>) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "package_clause" => {
                // `package auth` → extract `auth`
                let mut inner = child.walk();
                for pkg_child in child.children(&mut inner) {
                    if pkg_child.kind() == "package_identifier" {
                        if let Ok(name) = pkg_child.utf8_text(src) {
                            symbols.push(Symbol { name: name.to_string(), priority: 10 });
                        }
                    }
                }
            }
            "type_declaration" => {
                // `type TokenService struct { ... }`
                let mut inner = child.walk();
                for spec in child.children(&mut inner) {
                    if spec.kind() == "type_spec" {
                        if let Some(name) = field_text(&spec, "name", src) {
                            // Exported (capitalized) types get higher priority.
                            let pri = if name.starts_with(|c: char| c.is_uppercase()) { 9 } else { 6 };
                            symbols.push(Symbol { name: name.to_string(), priority: pri });
                        }
                    }
                }
            }
            "function_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    let pri = if name.starts_with(|c: char| c.is_uppercase()) { 7 } else { 5 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            "method_declaration" => {
                // Go methods with receivers — lower priority, but still top-level.
                if let Some(name) = field_text(&child, "name", src) {
                    let pri = if name.starts_with(|c: char| c.is_uppercase()) { 6 } else { 4 };
                    symbols.push(Symbol { name: name.to_string(), priority: pri });
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// C / C++
// ---------------------------------------------------------------------------

fn collect_c_cpp(root: &tree_sitter::Node, src: &[u8], symbols: &mut Vec<Symbol>) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "class_specifier" | "struct_specifier" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 9 });
                }
            }
            "function_definition" => {
                // The function name is nested in the declarator.
                if let Some(decl) = child.child_by_field_name("declarator") {
                    if let Some(name) = extract_identifier_from_declarator(&decl, src) {
                        symbols.push(Symbol { name, priority: 7 });
                    }
                }
            }
            "declaration" => {
                // Top-level variable/function declarations.
                if let Some(decl) = child.child_by_field_name("declarator") {
                    if decl.kind() == "function_declarator" {
                        if let Some(name) = extract_identifier_from_declarator(&decl, src) {
                            symbols.push(Symbol { name, priority: 7 });
                        }
                    }
                }
            }
            "namespace_definition" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 10 });
                }
            }
            _ => {}
        }
    }
}

/// Recursively descend a C/C++ declarator to find the identifier name.
/// Handles nested declarators like `(*func_ptr)(...)` and `ClassName::method(...)`.
fn extract_identifier_from_declarator(node: &tree_sitter::Node, src: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" => node.utf8_text(src).ok().map(|s| s.to_string()),
        "field_identifier" => node.utf8_text(src).ok().map(|s| s.to_string()),
        "qualified_identifier" | "scoped_identifier" => {
            // `ClassName::method` → take `method`
            if let Some(name_node) = node.child_by_field_name("name") {
                return name_node.utf8_text(src).ok().map(|s| s.to_string());
            }
            None
        }
        _ => {
            // Try the `declarator` field first, then fall back to scanning children.
            if let Some(inner) = node.child_by_field_name("declarator") {
                return extract_identifier_from_declarator(&inner, src);
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" || child.kind() == "field_identifier" {
                    return child.utf8_text(src).ok().map(|s| s.to_string());
                }
            }
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Regex fallback (for languages without tree-sitter grammars)
// ---------------------------------------------------------------------------

fn extract_code_regex(content: &str, style: CodeStyle) -> Option<String> {
    let patterns: Vec<&str> = match style {
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
        // tree-sitter-covered styles should never reach here, but just in case
        // the tree-sitter parse produces an empty result, provide regex patterns
        // so the caller still gets a useful name.
        CodeStyle::JsTs => vec![
            // exported class / interface (any name)
            r"(?m)^export\s+(?:default\s+)?(?:abstract\s+)?(?:class|interface)\s+([A-Za-z_][a-zA-Z0-9_]*)",
            // exported function (async or generator, any name)
            r"(?m)^export\s+(?:async\s+)?function\s*\*?\s*([a-zA-Z_][a-zA-Z0-9_]+)",
            // exported const / let with a non-trivial name
            r"(?m)^export\s+(?:const|let)\s+([a-zA-Z_][a-zA-Z0-9_]+)",
            // re-export alias: export { Foo as Bar } — use the public name
            r"(?m)\bexport\s*\{[^}]*\bas\s+([A-Za-z_][a-zA-Z0-9_]*)\s*[,}]",
        ],
        // Other tree-sitter-covered styles (Python, Rust, Java, Go, C/C++) have
        // reliable parsers; if they return nothing, a regex fallback adds little.
        _ => return None,
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
                    if !name.is_empty() && !is_noise_name(&name) && seen.insert(name.clone()) {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Python ───────────────────────────────────────────────────────────

    #[test]
    fn python_class_and_toplevel_fn() {
        let code = r#"
class UserAuthentication:
    def login(self, username, password):
        pass
    def logout(self):
        pass

def setup_logging():
    pass
"#;
        let result = extract_code(code, CodeStyle::Python).unwrap();
        // Should capture UserAuthentication (class, P9) and setup_logging (top-level fn, P7).
        // Should NOT capture login/logout (they are methods inside the class).
        assert!(result.contains("UserAuthentication"), "got: {result}");
        assert!(!result.contains("login"), "methods should not appear: {result}");
        assert!(!result.contains("logout"), "methods should not appear: {result}");
    }

    #[test]
    fn python_decorated_class() {
        let code = r#"
@dataclass
class Config:
    host: str
    port: int
"#;
        let result = extract_code(code, CodeStyle::Python).unwrap();
        assert!(result.contains("Config"), "got: {result}");
    }

    // ── JavaScript ───────────────────────────────────────────────────────

    #[test]
    fn js_exported_class_and_function() {
        let code = r#"
export class JWTValidator {
  validate(token) { return true; }
}

export function createToken(payload) {
  return sign(payload);
}
"#;
        let result = extract_code(code, CodeStyle::JsTs).unwrap();
        assert!(
            result.contains("JWTValidator"),
            "should capture exported class: {result}"
        );
    }

    #[test]
    fn js_const_uppercase() {
        let code = r#"
export const AppRouter = createRouter();
export const DefaultConfig = { timeout: 30 };

function helper() {}
"#;
        let result = extract_code(code, CodeStyle::JsTs).unwrap();
        assert!(
            result.contains("AppRouter") || result.contains("DefaultConfig"),
            "should capture uppercase const: {result}"
        );
    }

    // ── TypeScript ───────────────────────────────────────────────────────

    #[test]
    fn ts_interface_and_type() {
        let code = r#"
interface ApiResponse<T> {
  data: T;
  error?: string;
}

type UserId = string;

export async function fetchUsers(): Promise<ApiResponse<User[]>> {
  return fetch('/users');
}
"#;
        let result = extract_code(code, CodeStyle::JsTs).unwrap();
        assert!(
            result.contains("ApiResponse") || result.contains("fetchUsers"),
            "should capture TS interface/function: {result}"
        );
    }

    // ── Rust ─────────────────────────────────────────────────────────────

    #[test]
    fn rust_pub_vs_private() {
        let code = r#"
pub struct Config {
    host: String,
}

impl Config {
    pub fn from_env() -> Self {
        Config { host: String::new() }
    }

    fn validate(&self) -> bool {
        true
    }
}

fn main() {
    let cfg = Config::from_env();
}
"#;
        let result = extract_code(code, CodeStyle::Rust).unwrap();
        // Config (pub struct, P9) should appear, main should be filtered as noise.
        assert!(result.contains("Config"), "got: {result}");
        assert!(!result.contains("main"), "main should be filtered: {result}");
    }

    #[test]
    fn rust_mod_highest_priority() {
        let code = r#"
pub mod authentication;

pub struct TokenParser {
    inner: Vec<u8>,
}

pub fn parse(input: &str) -> Token {
    todo!()
}
"#;
        let result = extract_code(code, CodeStyle::Rust).unwrap();
        // pub mod (P10) should appear first.
        assert!(result.starts_with("authentication"), "pub mod should be first: {result}");
    }

    // ── Java ─────────────────────────────────────────────────────────────

    #[test]
    fn java_class_not_methods() {
        let code = r#"
package com.example.payment;

public class PaymentProcessor {
    private PaymentGateway gateway;

    public PaymentResult process(Order order) {
        return gateway.charge(order);
    }

    private void validate(Order order) {
        // validation logic
    }
}
"#;
        let result = extract_code(code, CodeStyle::JavaLike).unwrap();
        // Should capture PaymentProcessor (class, P9) and payment (package, P10).
        // Should NOT capture process/validate (methods inside class).
        assert!(
            result.contains("PaymentProcessor") || result.contains("payment"),
            "got: {result}"
        );
        assert!(!result.contains("process"), "methods should not appear: {result}");
        assert!(!result.contains("validate"), "methods should not appear: {result}");
    }

    // ── Go ───────────────────────────────────────────────────────────────

    #[test]
    fn go_package_and_exported_type() {
        let code = r#"
package auth

type TokenService struct {
    secret string
}

func (s *TokenService) Generate(claims Claims) string {
    return ""
}

func init() {
    // setup
}
"#;
        let result = extract_code(code, CodeStyle::Go).unwrap();
        // auth (package, P10) + TokenService (exported type, P9)
        assert!(result.contains("auth"), "package should appear: {result}");
        assert!(result.contains("TokenService"), "exported type should appear: {result}");
        assert!(!result.contains("init"), "init should be filtered: {result}");
    }

    #[test]
    fn go_unexported_lower_priority() {
        let code = r#"
package utils

type helper struct{}

func doWork() {}
"#;
        let result = extract_code(code, CodeStyle::Go).unwrap();
        assert!(result.contains("utils"), "package should appear: {result}");
    }

    // ── C ────────────────────────────────────────────────────────────────

    #[test]
    fn c_struct_and_function() {
        let code = r#"
struct HashTable {
    int size;
    void **entries;
};

int hash_insert(struct HashTable *ht, const char *key, void *value) {
    return 0;
}
"#;
        let result = extract_code(code, CodeStyle::CFamily).unwrap();
        assert!(
            result.contains("HashTable") || result.contains("hash_insert"),
            "got: {result}"
        );
    }

    // ── C++ ──────────────────────────────────────────────────────────────

    #[test]
    fn cpp_class_detection() {
        let code = r#"
#include <string>

namespace network {

class HttpClient {
public:
    void get(const std::string& url);
private:
    std::string base_url;
};

}
"#;
        let result = extract_code(code, CodeStyle::CFamily).unwrap();
        assert!(
            result.contains("network") || result.contains("HttpClient"),
            "got: {result}"
        );
    }

    // ── Regex fallback ──────────────────────────────────────────────────

    #[test]
    fn shell_regex_fallback() {
        let code = r#"
deploy() {
    echo "deploying..."
}

function cleanup {
    echo "cleaning up..."
}
"#;
        let result = extract_code(code, CodeStyle::Shell).unwrap();
        assert!(
            result.contains("deploy") || result.contains("cleanup"),
            "regex fallback should work: {result}"
        );
    }

    #[test]
    fn swift_regex_fallback() {
        let code = r#"
public class NetworkManager {
    func fetchData() { }
}

public struct APIEndpoint {
    let path: String
}
"#;
        let result = extract_code(code, CodeStyle::Swift).unwrap();
        assert!(
            result.contains("NetworkManager") || result.contains("APIEndpoint"),
            "regex fallback should work: {result}"
        );
    }

    #[test]
    fn ruby_regex_fallback() {
        let code = r#"
class UserRepository
  def find_by_id(id)
    # ...
  end
end
"#;
        let result = extract_code(code, CodeStyle::Ruby).unwrap();
        assert!(
            result.contains("UserRepository"),
            "regex fallback should work: {result}"
        );
    }
}
