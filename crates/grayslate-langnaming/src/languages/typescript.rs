use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "typescript",
        extension: "ts",
        extract: Extractor::Custom(extract_ts),
    }
}

/// TypeScript naming: React detection → regex fallback.
fn extract_ts(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static REACT_COMPONENT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^export\s+(?:default\s+)?function\s+([A-Z][a-zA-Z0-9]+)").unwrap()
    });
    static REACT_ARROW: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:export\s+(?:default\s+)?)?const\s+([A-Z][a-zA-Z0-9]+)\s*(?::\s*React\.FC|:\s*FC)?(?:<[^>]*>)?\s*=\s*(?:\([^)]*\)|[a-zA-Z_]\w*)\s*=>").unwrap()
    });

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

    use crate::code::extract_with_regex;
    const PATTERNS: &[(&str, u8)] = &[
        // Exported declarations — highest priority
        (r"(?m)^export\s+(?:default\s+)?(?:abstract\s+)?class\s+([A-Z]\w+)", 9),
        (r"(?m)^export\s+(?:default\s+)?(?:async\s+)?function\s+([a-zA-Z_]\w+)", 8),
        (r"(?m)^export\s+(?:interface|type)\s+([A-Z]\w+)", 8),
        (r"(?m)^export\s+(?:default\s+)?enum\s+([A-Z]\w+)", 8),
        // Barrel re-exports: export { Root as Badge } → capture PascalCase alias
        (r"(?m)^export\s*\{[^}]*\bas\s+([A-Z]\w+)", 8),
        (r"(?m)^export\s+(?:default\s+)?(?:const|let|var)\s+([a-zA-Z_$]\w+)", 7),
        // export default <identifier> (not followed by function/class keyword)
        (r"(?m)^export\s+default\s+([A-Z]\w+)\s*;?\s*$", 7),
        // Non-exported module-level declarations — lower priority
        (r"(?m)^(?:declare\s+)?(?:abstract\s+)?class\s+([A-Z]\w+)", 6),
        (r"(?m)^(?:declare\s+)?interface\s+([A-Z]\w+)", 6),
        (r"(?m)^(?:declare\s+)?type\s+([A-Z]\w+)", 6),
        (r"(?m)^(?:declare\s+)?enum\s+([A-Z]\w+)", 6),
        // Namespace declarations — context like package
        (r"(?m)^(?:declare\s+)?(?:export\s+)?namespace\s+([A-Z]\w+)", 5),
        (r"(?m)^(?:async\s+)?function\s+([a-zA-Z_]\w+)", 5),
        (r"(?m)^const\s+([a-zA-Z_$]\w+)\s*(?::\s*\w|=)", 4),
    ];
    extract_with_regex(content, PATTERNS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::slugify;

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

    #[test]
    fn non_exported_function() {
        let src = "function processData(items: Item[]): Result {\n  return items.map(transform);\n}";
        let n = name(src).unwrap();
        assert!(n.contains("process-data"), "non-exported function: {n}");
    }

    #[test]
    fn non_exported_class() {
        let src = "class UserService {\n  private db: Database;\n  async getUser(id: string) { return this.db.find(id); }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("user-service"), "non-exported class: {n}");
    }

    #[test]
    fn standalone_const() {
        let src = "const MAX_RETRIES = 3;\nconst fetchConfig = { timeout: 5000 };";
        let n = name(src).unwrap();
        assert!(n.contains("max-retries") || n.contains("fetch-config"), "standalone const: {n}");
    }

    #[test]
    fn declare_interface() {
        let src = "declare interface ComponentCustomProperties {\n  $auth: AuthService;\n}\n\ndeclare type AppConfig = Record<string, unknown>;";
        let n = name(src).unwrap();
        assert!(n.contains("component-custom-properties"), "declare interface: {n}");
    }

    #[test]
    fn enum_extraction() {
        let src = "export enum OrderStatus {\n  Pending,\n  Processing,\n  Completed\n}";
        let n = name(src).unwrap();
        assert!(n.contains("order-status"), "enum: {n}");
    }

    #[test]
    fn export_default_identifier() {
        let src = "class AppComponent {\n  render() { return null; }\n}\nexport default AppComponent;";
        let n = name(src).unwrap();
        assert!(n.contains("app-component"), "export default identifier: {n}");
    }

    #[test]
    fn dollar_prefix_const() {
        let src = "export const $teamList = atom<Team[]>([]);\nexport const $currentTeam = atom<Team | null>(null);";
        let n = name(src).unwrap();
        assert!(n.contains("team"), "dollar-prefix const captured: {n}");
    }

    #[test]
    fn abstract_class() {
        let src = "export abstract class BaseRepository<T> {\n  abstract findById(id: string): Promise<T>;\n  abstract save(entity: T): Promise<void>;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("base-repository"), "abstract class: {n}");
    }

    #[test]
    fn namespace_declaration() {
        let src = "declare namespace Express {\n  interface Request {\n    user?: User;\n  }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("express"), "namespace: {n}");
    }
}
