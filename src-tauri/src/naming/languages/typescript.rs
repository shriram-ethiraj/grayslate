use super::NamingDefinition;
use crate::naming::code::extract_with_tree_sitter;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "typescript",
        extension: "ts",
        extract: extract_ts,
    }
}

/// TypeScript naming: React detection → TS tree-sitter → JS fallback.
fn extract_ts(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    // React/component detection (before tree-sitter for cleaner names)
    static REACT_COMPONENT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^export\s+(?:default\s+)?function\s+([A-Z][a-zA-Z0-9]+)").unwrap()
    });
    static REACT_ARROW: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:export\s+(?:default\s+)?)?const\s+([A-Z][a-zA-Z0-9]+)\s*(?::\s*React\.FC|:\s*FC)?(?:<[^>]*>)?\s*=\s*(?:\([^)]*\)|[a-zA-Z_]\w*)\s*=>").unwrap()
    });

    // Detect React/JSX patterns — broader detection including JSX syntax
    if content.contains("React") || content.contains("JSX") || content.contains("useState")
        || content.contains(": React.FC") || content.contains(": FC<")
        || content.contains("className=") || content.contains("useRef")
        || content.contains("useCallback") || content.contains("useMemo")
        || super::javascript::has_jsx_tags(content)
    {
        if let Some(cap) = REACT_COMPONENT.captures(content) {
            return Some(cap[1].to_string());
        }
        if let Some(cap) = REACT_ARROW.captures(content) {
            return Some(cap[1].to_string());
        }
    }

    // Try TypeScript grammar first
    if let Some(result) = extract_with_tree_sitter(
        content,
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        super::javascript::collect_js_nodes,
    ) {
        return Some(result);
    }

    // Fall back to JS grammar
    if let Some(result) = extract_with_tree_sitter(
        content,
        tree_sitter_javascript::LANGUAGE,
        super::javascript::collect_js_nodes,
    ) {
        return Some(result);
    }

    // Final regex fallback for patterns tree-sitter misses
    use crate::naming::code::extract_with_regex;
    const PATTERNS: &[(&str, u8)] = &[
        (r"(?m)^export\s+(?:default\s+)?class\s+([A-Z]\w+)", 9),
        (r"(?m)^export\s+(?:default\s+)?(?:async\s+)?function\s+([a-zA-Z_]\w+)", 8),
        (r"(?m)^export\s+(?:default\s+)?(?:const|let|var)\s+([a-zA-Z_]\w+)", 7),
        (r"(?m)^(?:interface|type)\s+([A-Z]\w+)", 8),
        (r"(?m)^class\s+([A-Z]\w+)", 8),
    ];
    extract_with_regex(content, PATTERNS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_ts(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn react_fc() {
        let src = "import React from 'react';\nexport default function Dashboard({ data }: Props) {\n  return <div>{data.map(d => <Card key={d.id} />)}</div>;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("dashboard"), "got: {n}");
    }

    #[test]
    fn interface_and_type() {
        let src = "export interface UserState {\n  name: string;\n  age: number;\n}\nexport type UserAction = 'login' | 'logout';";
        let n = name(src).unwrap();
        assert!(n.contains("user-state"), "got: {n}");
    }

    #[test]
    fn ts_interface_and_fn() {
        let src = "interface ApiResponse<T> {\n  data: T;\n  error?: string;\n}\ntype UserId = string;\nexport async function fetchUsers(): Promise<ApiResponse<User[]>> { return fetch('/users'); }";
        let n = name(src).unwrap();
        assert!(
            n.contains("api-response") || n.contains("fetch-users"),
            "should capture TS interface/function: {n}"
        );
    }
}
