use super::NamingDefinition;
use crate::naming::code::{extract_with_tree_sitter, field_text, Symbol};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "javascript",
        extension: "js",
        extract: extract_js,
    }
}

/// JavaScript naming: config/React detection → tree-sitter → regex fallback.
fn extract_js(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    // Config-like patterns (often lack named exports)
    static CONFIG_MODULE_EXPORTS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^module\.exports\s*=").unwrap()
    });
    static CONFIG_DEFINE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:export\s+default\s+)?(?:define(?:Config|Plugin|Preset)|createConfig|makeConfig)\s*\(").unwrap()
    });
    static ESLINT_CONFIG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?s)\b(?:rules|extends|plugins|overrides)\b.*\b(?:rules|extends|plugins|overrides)\b"#).unwrap()
    });

    // React/Vue component detection (PascalCase export)
    static REACT_COMPONENT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^export\s+(?:default\s+)?function\s+([A-Z][a-zA-Z0-9]+)").unwrap()
    });
    static REACT_ARROW: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:export\s+(?:default\s+)?)?const\s+([A-Z][a-zA-Z0-9]+)\s*=\s*(?:\([^)]*\)|[a-zA-Z_]\w*)\s*=>").unwrap()
    });

    // CommonJS named exports: exports.X = ...
    static COMMONJS_EXPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^exports\.([a-zA-Z_]\w+)\s*=").unwrap()
    });

    // Check for config-like patterns
    if CONFIG_DEFINE.is_match(content) || ESLINT_CONFIG.is_match(content) {
        let name_re = regex::Regex::new(r#"(?m)name\s*:\s*["']([^"']+)["']"#).ok();
        if let Some(re) = &name_re {
            if let Some(cap) = re.captures(content) {
                return Some(cap[1].to_string());
            }
        }
        if CONFIG_MODULE_EXPORTS.is_match(content) {
            // Fall through to tree-sitter
        }
    }

    // React/JSX component detection — broader: also detect JSX syntax
    // (className=, onClick=, <Component, etc.)
    let has_jsx = content.contains("React") || content.contains("jsx")
        || content.contains("useState") || content.contains("useEffect")
        || content.contains("className=") || content.contains("onClick=")
        || content.contains("useRef") || content.contains("useCallback")
        || content.contains("useMemo") || content.contains("useContext")
        || has_jsx_tags(content);

    if has_jsx {
        if let Some(cap) = REACT_COMPONENT.captures(content) {
            return Some(cap[1].to_string());
        }
        if let Some(cap) = REACT_ARROW.captures(content) {
            return Some(cap[1].to_string());
        }
    }

    // Tree-sitter (JS grammar)
    if let Some(result) = extract_with_tree_sitter(
        content,
        tree_sitter_javascript::LANGUAGE,
        collect_js_nodes,
    ) {
        return Some(result);
    }

    // Try TS grammar as fallback (handles some modern JS)
    if let Some(result) = extract_with_tree_sitter(
        content,
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        collect_js_nodes,
    ) {
        return Some(result);
    }

    // CommonJS: module.exports = { name } or exports.X = ...
    if CONFIG_MODULE_EXPORTS.is_match(content) {
        // Try to extract a name from the object being exported
        let name_re = regex::Regex::new(r#"(?m)module\.exports\s*=\s*(?:class|function)\s+([A-Za-z_]\w+)"#).ok();
        if let Some(re) = &name_re {
            if let Some(cap) = re.captures(content) {
                return Some(cap[1].to_string());
            }
        }
    }
    // exports.X = ... → collect exported names
    {
        let mut cjs_names: Vec<String> = Vec::new();
        for cap in COMMONJS_EXPORT_RE.captures_iter(content).take(4) {
            let name = cap[1].to_string();
            if !crate::naming::code::is_noise_name(&name) {
                cjs_names.push(name);
            }
        }
        if !cjs_names.is_empty() {
            return Some(cjs_names.join("-"));
        }
    }

    // Final regex fallback
    use crate::naming::code::extract_with_regex;
    const PATTERNS: &[(&str, u8)] = &[
        (r"(?m)^export\s+(?:default\s+)?class\s+([A-Z]\w+)", 9),
        (r"(?m)^export\s+(?:default\s+)?(?:async\s+)?function\s+([a-zA-Z_]\w+)", 8),
        (r"(?m)^export\s+(?:default\s+)?(?:const|let|var)\s+([a-zA-Z_]\w+)", 7),
        (r"(?m)^export\s+default\s+(\w+)\s*\(", 6),
    ];
    extract_with_regex(content, PATTERNS)
}

/// Heuristic: detect JSX-like self-closing or component tags in content.
pub(super) fn has_jsx_tags(content: &str) -> bool {
    use regex::Regex;
    use std::sync::LazyLock;
    // Match `<ComponentName` (PascalCase) or `<Component />` patterns
    static JSX_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"<[A-Z][a-zA-Z0-9]+[\s/>]").unwrap()
    });
    JSX_TAG_RE.is_match(content)
}

/// Collect JS/TS top-level symbols from tree-sitter nodes.
pub(super) fn collect_js_nodes(
    root: &tree_sitter::Node,
    src: &[u8],
    symbols: &mut Vec<Symbol>,
) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "export_statement" => collect_export_children(&child, src, symbols),
            "class_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 8 });
                }
            }
            "function_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 6 });
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                collect_lexical_decl(&child, src, symbols, 5);
            }
            _ => {}
        }
    }
}

fn collect_export_children(
    export_node: &tree_sitter::Node,
    src: &[u8],
    symbols: &mut Vec<Symbol>,
) {
    let mut cursor = export_node.walk();
    for child in export_node.children(&mut cursor) {
        match child.kind() {
            "class_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 9 });
                }
            }
            "function_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 8 });
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                collect_lexical_decl(&child, src, symbols, 7);
            }
            // TS-specific: interface, type_alias, enum
            "interface_declaration" | "type_alias_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 9 });
                }
            }
            "enum_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 8 });
                }
            }
            // Named re-exports: export { Root as Badge, badgeVariants }
            "export_clause" => {
                collect_export_clause(&child, src, symbols);
            }
            // export default defineConfig({...}) or export default function X() {}
            "call_expression" => {
                // Extract callee name: defineConfig → "defineConfig"
                if let Some(func_node) = child.child_by_field_name("function") {
                    if let Ok(name) = func_node.utf8_text(src) {
                        let clean = name.split('.').last().unwrap_or(name);
                        if !clean.is_empty() {
                            symbols.push(Symbol { name: clean.to_string(), priority: 6 });
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Extract names from `export { X as Y, Z }` clauses.
fn collect_export_clause(
    clause_node: &tree_sitter::Node,
    src: &[u8],
    symbols: &mut Vec<Symbol>,
) {
    let mut cursor = clause_node.walk();
    for child in clause_node.children(&mut cursor) {
        if child.kind() == "export_specifier" {
            // Prefer alias (as Name) over original name
            let name_node = child.child_by_field_name("alias")
                .or_else(|| child.child_by_field_name("name"));
            if let Some(n) = name_node {
                if let Ok(name) = n.utf8_text(src) {
                    let first = name.chars().next().unwrap_or('_');
                    if first.is_uppercase() {
                        symbols.push(Symbol { name: name.to_string(), priority: 7 });
                    }
                }
            }
        }
    }
}

/// Extract `const` names from lexical declarations.
/// Captures both uppercase (components/classes) and camelCase (exported config).
pub(super) fn collect_lexical_decl(
    decl_node: &tree_sitter::Node,
    src: &[u8],
    symbols: &mut Vec<Symbol>,
    priority: u8,
) {
    let mut cursor = decl_node.walk();
    for child in decl_node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            if let Some(name) = field_text(&child, "name", src) {
                if !name.is_empty() {
                    let first = name.chars().next().unwrap_or('_');
                    // Uppercase: high priority (class/component-like)
                    // camelCase: lower priority but still captured
                    let p = if first.is_uppercase() { priority } else { priority.saturating_sub(2) };
                    symbols.push(Symbol { name: name.to_string(), priority: p });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_js(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn react_component() {
        let src = "import React from 'react';\nexport default function UserProfile({ userId }) {\n  const [user, setUser] = useState(null);\n  return <div>{user?.name}</div>;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("user-profile"), "got: {n}");
    }

    #[test]
    fn arrow_component() {
        let src = "import { useState } from 'react';\nexport const TodoList = (props) => {\n  return <ul></ul>;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("todo-list"), "got: {n}");
    }

    #[test]
    fn jsx_without_react_import() {
        // No React import, but has JSX syntax with PascalCase tags
        let src = "export default function SearchBar({ onSearch }) {\n  return <div className=\"search\">\n    <Input placeholder=\"Search...\" />\n  </div>;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("search-bar"), "JSX without React import: {n}");
    }

    #[test]
    fn commonjs_exports() {
        // Pure CommonJS — no const/let declarations that tree-sitter would pick up
        let src = "exports.tokenize = function(input) {};\nexports.formatOutput = function(tokens) {};";
        let n = name(src).unwrap();
        assert!(n.contains("tokenize"), "CommonJS exports: {n}");
    }

    #[test]
    fn regular_class() {
        let src = "export class EventEmitter {\n  constructor() {}\n  emit(event) {}\n}";
        let n = name(src).unwrap();
        assert!(n.contains("event-emitter"), "got: {n}");
    }

    #[test]
    fn exported_function_and_const() {
        let src = "export class JWTValidator {\n  validate(token) { return true; }\n}\nexport function createToken(payload) { return sign(payload); }";
        let n = name(src).unwrap();
        assert!(n.contains("jwtvalidator") || n.contains("jwt-validator"), "got: {n}");
    }
}
