use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "javascript",
        extension: "js",
        extract: Extractor::Custom(extract_js),
    }
}

/// JavaScript naming: config detection → React/JSX detection → CommonJS → regex.
fn extract_js(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static CONFIG_MODULE_EXPORTS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^module\.exports\s*=").unwrap()
    });
    static CONFIG_DEFINE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:export\s+default\s+)?(?:define(?:Config|Plugin|Preset)|createConfig|makeConfig)\s*\(").unwrap()
    });
    static ESLINT_CONFIG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?s)\b(?:rules|extends|plugins|overrides)\b.*\b(?:rules|extends|plugins|overrides)\b"#).unwrap()
    });

    static REACT_COMPONENT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^export\s+(?:default\s+)?function\s+([A-Z][a-zA-Z0-9]+)").unwrap()
    });
    static REACT_ARROW: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:export\s+(?:default\s+)?)?const\s+([A-Z][a-zA-Z0-9]+)\s*=\s*(?:\([^)]*\)|[a-zA-Z_]\w*)\s*=>").unwrap()
    });

    static COMMONJS_EXPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^exports\.([a-zA-Z_]\w+)\s*=").unwrap()
    });

    // Config-like patterns
    if CONFIG_DEFINE.is_match(content) || ESLINT_CONFIG.is_match(content) {
        let name_re = regex::Regex::new(r#"(?m)name\s*:\s*["']([^"']+)["']"#).ok();
        if let Some(re) = &name_re {
            if let Some(cap) = re.captures(content) {
                return Some(cap[1].to_string());
            }
        }
    }

    // React/JSX component detection
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

    // CommonJS: module.exports = class/function Name
    if CONFIG_MODULE_EXPORTS.is_match(content) {
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
            if !crate::code::is_noise_name(&name) {
                cjs_names.push(name);
            }
        }
        if !cjs_names.is_empty() {
            return Some(cjs_names.join("-"));
        }
    }

    // Regex fallback
    use crate::code::extract_with_regex;
    const PATTERNS: &[(&str, u8)] = &[
        // Exported declarations — highest priority
        (r"(?m)^export\s+(?:default\s+)?(?:abstract\s+)?class\s+([A-Z]\w+)", 9),
        (r"(?m)^export\s+(?:default\s+)?(?:async\s+)?function\s+([a-zA-Z_]\w+)", 8),
        // Barrel re-exports: export { Root as Badge } → capture PascalCase alias
        (r"(?m)^export\s*\{[^}]*\bas\s+([A-Z]\w+)", 8),
        (r"(?m)^export\s+(?:default\s+)?(?:const|let|var)\s+([a-zA-Z_$]\w+)", 7),
        // export default <identifier>; (standalone reference)
        (r"(?m)^export\s+default\s+([A-Z]\w+)\s*;?\s*$", 7),
        (r"(?m)^export\s+default\s+(\w+)\s*\(", 6),
        // Non-exported module-level declarations — lower priority
        (r"(?m)^class\s+([A-Z]\w+)", 6),
        (r"(?m)^(?:async\s+)?function\s+([a-zA-Z_]\w+)", 5),
        (r"(?m)^const\s+([a-zA-Z_$]\w+)\s*=", 4),
    ];
    extract_with_regex(content, PATTERNS)
}

/// Heuristic: detect JSX-like PascalCase component tags.
pub(super) fn has_jsx_tags(content: &str) -> bool {
    use regex::Regex;
    use std::sync::LazyLock;
    static JSX_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"<[A-Z][a-zA-Z0-9]+[\s/>]").unwrap()
    });
    JSX_TAG_RE.is_match(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::slugify;

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
        let src = "export default function SearchBar({ onSearch }) {\n  return <div className=\"search\">\n    <Input placeholder=\"Search...\" />\n  </div>;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("search-bar"), "JSX without React import: {n}");
    }

    #[test]
    fn commonjs_exports() {
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

    #[test]
    fn non_exported_function() {
        let src = "function App() {\n  return createElement('div', null, 'Hello');\n}";
        let n = name(src);
        // "App" is noise, but function still captures
        assert!(n.is_none() || n.as_deref() == Some("app"), "non-exported function: {n:?}");
    }

    #[test]
    fn non_exported_class() {
        let src = "class DataProcessor {\n  process(data) { return data.map(x => x * 2); }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("data-processor"), "non-exported class: {n}");
    }

    #[test]
    fn standalone_const_assignment() {
        let src = "const connectionPool = createPool({ host: 'localhost' });\nconst maxRetries = 3;";
        let n = name(src).unwrap();
        assert!(n.contains("connection-pool") || n.contains("max-retries"), "standalone const: {n}");
    }

    #[test]
    fn export_default_identifier() {
        let src = "class Component {\n  render() {}\n}\nexport default Component;";
        let n = name(src).unwrap();
        assert!(n.contains("component"), "export default identifier: {n}");
    }

    #[test]
    fn barrel_reexport() {
        let src = "export { Root as Badge } from './badge';\nexport { Root as Button } from './button';";
        let n = name(src).unwrap();
        assert!(n.contains("badge"), "barrel re-export: {n}");
    }

    #[test]
    fn eslint_config_with_name() {
        let src = "export default defineConfig({\n  name: 'my-eslint-config',\n  rules: { 'no-console': 'warn' },\n  plugins: ['react'],\n});";
        let n = name(src).unwrap();
        assert!(n.contains("eslint-config"), "config name: {n}");
    }
}
