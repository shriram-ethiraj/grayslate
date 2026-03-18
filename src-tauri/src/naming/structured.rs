use super::model::MAX_TOKENS;

// ---------------------------------------------------------------------------
// Per-format extractors
// ---------------------------------------------------------------------------

// ===== CSV =================================================================

/// Returns `true` if `col` (already lowercased) is a noise/non-semantic column.
fn is_noise_csv_column(col: &str) -> bool {
    // Exact identity / surrogate-key columns.
    const EXACT_ID: &[&str] = &[
        "id", "_id", "uuid", "guid", "index", "idx", "row", "row_number",
        "row_num", "key", "pk", "rowid",
    ];
    // Timestamp / audit columns.
    const EXACT_TS: &[&str] = &[
        "created_at", "updated_at", "deleted_at", "timestamp", "date", "time",
        "datetime", "created", "updated", "modified", "modified_at",
    ];
    // Coordinate columns.
    const EXACT_COORD: &[&str] = &[
        "latitude", "longitude", "lat", "lng", "lon", "x", "y",
    ];

    if EXACT_ID.contains(&col) || EXACT_TS.contains(&col) || EXACT_COORD.contains(&col) {
        return true;
    }
    // Columns ending in `_id` (e.g. `employee_id`, `order_id`).
    if col.len() > 3 && col.ends_with("_id") {
        return true;
    }
    // Generic positional columns: col1, column_a, field3, unnamed_0, var1, v2.
    if let Ok(re) = regex::Regex::new(
        r"^(col|column|field|unnamed|var|v)_?[\d]+$"
    ) {
        if re.is_match(col) {
            return true;
        }
    }
    if let Ok(re) = regex::Regex::new(
        r"^(col|column|field|unnamed|var|v)_?[a-z]$"
    ) {
        if re.is_match(col) {
            return true;
        }
    }
    false
}

/// CSV: take first MAX_TOKENS non-noise header column names from the first line.
pub(super) fn extract_csv(content: &str) -> Option<String> {
    let first_line = content.lines().next()?.trim();
    if first_line.is_empty() {
        return None;
    }

    let delimiter = if first_line.matches('\t').count() > first_line.matches(',').count() {
        '\t'
    } else {
        ','
    };

    let all_headers: Vec<&str> = first_line
        .split(delimiter)
        .map(|h| h.trim().trim_matches('"').trim_matches('\'').trim())
        .filter(|h| !h.is_empty())
        .collect();

    if all_headers.is_empty() {
        return None;
    }

    let semantic: Vec<&str> = all_headers
        .iter()
        .filter(|h| !is_noise_csv_column(&h.to_lowercase()))
        .copied()
        .collect();

    // If every column is noise, return None (let timestamp fallback handle it).
    if semantic.is_empty() {
        return None;
    }

    let tokens: Vec<&str> = semantic.into_iter().take(MAX_TOKENS).collect();
    Some(tokens.join("-"))
}

// ===== JSON ================================================================

/// JSON noise keys (case-insensitive exact match).
const JSON_NOISE_KEYS: &[&str] = &[
    "id", "_id", "$schema", "$ref", "$defs", "$id", "definitions",
    "meta", "metadata", "timestamp", "created_at", "updated_at", "deleted_at",
];

/// `type` is noise only when its value is a generic type name.
const JSON_GENERIC_TYPE_VALUES: &[&str] = &[
    "object", "array", "string", "number", "integer", "boolean", "null",
];

fn is_json_noise_key(key: &str, value: Option<&serde_json::Value>) -> bool {
    let lower = key.to_lowercase();
    if JSON_NOISE_KEYS.contains(&lower.as_str()) {
        return true;
    }
    // `type` is noise only when its value is a generic type string.
    if lower == "type" {
        if let Some(serde_json::Value::String(s)) = value {
            if JSON_GENERIC_TYPE_VALUES.contains(&s.to_lowercase().as_str()) {
                return true;
            }
        }
    }
    false
}

/// Extract a short string value from a JSON key (for `name`, `title`, `error`).
fn json_short_string_value(value: &serde_json::Value) -> Option<String> {
    if let serde_json::Value::String(s) = value {
        let trimmed = s.trim();
        if !trimmed.is_empty() && trimmed.len() <= 60 {
            return Some(trimmed.to_string());
        }
    }
    None
}

/// Known-pattern detection: returns Some(stem) if the object matches a known
/// schema shape (package.json, OpenAPI, tsconfig, GeoJSON, JSON Schema).
fn json_detect_known_pattern(obj: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let has = |k: &str| obj.contains_key(k);
    let str_val = |k: &str| obj.get(k).and_then(|v| v.as_str());

    // package.json: {name, version, dependencies|scripts}
    if has("name") && has("version") && (has("dependencies") || has("scripts")) {
        if let Some(name) = str_val("name") {
            if !name.is_empty() && name.len() <= 60 {
                return Some(name.to_string());
            }
        }
    }

    // JSON Schema: {$schema} with `title`
    if has("$schema") {
        if let Some(title) = str_val("title") {
            if !title.is_empty() && title.len() <= 60 {
                return Some(title.to_string());
            }
        }
    }

    // OpenAPI / Swagger: {swagger|openapi} with info.title
    if has("swagger") || has("openapi") {
        if let Some(info) = obj.get("info").and_then(|v| v.as_object()) {
            if let Some(title) = info.get("title").and_then(|v| v.as_str()) {
                if !title.is_empty() && title.len() <= 60 {
                    return Some(title.to_string());
                }
            }
        }
    }

    // tsconfig: {compilerOptions}
    if has("compilerOptions") {
        return Some("tsconfig".to_string());
    }

    // GeoJSON: {type: "FeatureCollection"}
    if str_val("type") == Some("FeatureCollection") {
        return Some("feature-collection".to_string());
    }

    None
}

/// JSON: semantic value extraction + pattern detection + noise filtering.
pub(super) fn extract_json(content: &str) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Try full parse first.
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return json_extract_from_value(&value);
    }

    // Partial parse via regex: find `"key":` patterns near the start.
    let re = regex::Regex::new(r#""([a-zA-Z_$][a-zA-Z0-9_$]*?)"\s*:"#).ok()?;
    let keys: Vec<String> = re
        .captures_iter(trimmed)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .filter(|k| !is_json_noise_key(k, None))
        .take(MAX_TOKENS)
        .collect();

    if keys.is_empty() {
        None
    } else {
        Some(keys.join("-"))
    }
}

fn json_extract_from_value(value: &serde_json::Value) -> Option<String> {
    let obj = match value {
        serde_json::Value::Object(map) => Some(map),
        serde_json::Value::Array(arr) => arr.iter().find_map(|v| v.as_object()),
        _ => None,
    }?;

    // 1) Known pattern detection (highest priority).
    if let Some(stem) = json_detect_known_pattern(obj) {
        return Some(stem);
    }

    // 2) Collect non-noise keys and attempt value extraction for semantic keys.
    let mut tokens: Vec<String> = Vec::new();
    for (key, val) in obj.iter() {
        if tokens.len() >= MAX_TOKENS {
            break;
        }
        if is_json_noise_key(key, Some(val)) {
            continue;
        }
        // For `name`, `title`, `error`: prefer the VALUE over the key name.
        let lower = key.to_lowercase();
        if matches!(lower.as_str(), "name" | "title" | "error") {
            if let Some(s) = json_short_string_value(val) {
                tokens.push(s);
                continue;
            }
        }
        tokens.push(key.clone());
    }

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join("-"))
    }
}

// ===== YAML ================================================================

/// YAML: extract first few `key:` lines from the bounded sample.
pub(super) fn extract_yaml(content: &str) -> Option<String> {
    // Skip document markers and comments; extract `key:` patterns.
    let re = regex::Regex::new(r"^([a-zA-Z_][a-zA-Z0-9_\-]*)[\s]*:").ok()?;
    let keys: Vec<String> = content
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#') && !t.starts_with("---") && !t.starts_with("...")
        })
        .filter_map(|l| {
            re.captures(l)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        })
        .take(MAX_TOKENS)
        .collect();

    if keys.is_empty() {
        None
    } else {
        Some(keys.join("-"))
    }
}

// ===== TOML ================================================================

/// Noise TOML section names (case-insensitive).
fn is_toml_noise_section(name: &str) -> bool {
    let lower = name.to_lowercase();
    matches!(
        lower.as_str(),
        "build-system" | "build-dependencies"
    ) || lower.starts_with("profile.")
      || lower.starts_with("target.")
}

/// Helper: read a string value from a taplo DOM node.
fn taplo_str_value(node: &taplo::dom::Node) -> Option<String> {
    node.as_str().map(|s| s.value().to_string())
}

/// Helper: look up `name = "..."` inside a taplo table node and return the
/// string value if short enough.
fn taplo_table_name_value(node: &taplo::dom::Node) -> Option<String> {
    let val = taplo_str_value(&node.get("name"))?;
    if !val.is_empty() && val.len() <= 60 {
        Some(val)
    } else {
        None
    }
}

/// Detect known TOML project patterns and return a stem.
fn toml_detect_known_pattern(root: &taplo::dom::Node) -> Option<String> {
    let root_table = root.as_table()?;
    let entries = root_table.entries().read();
    let has_section = |name: &str| {
        entries
            .iter()
            .any(|(k, _)| k.value() == name)
    };

    // Cargo.toml: [package] with name
    if has_section("package") {
        if let Some(name) = taplo_table_name_value(&root.get("package")) {
            return Some(name);
        }
    }

    // pyproject.toml: [project] with name
    if has_section("project") {
        if let Some(name) = taplo_table_name_value(&root.get("project")) {
            return Some(name);
        }
    }

    // Poetry: [tool.poetry] with name
    let tool_node = root.get("tool");
    if !tool_node.is_invalid() {
        let poetry_node = tool_node.get("poetry");
        if !poetry_node.is_invalid() {
            if let Some(name) = taplo_table_name_value(&poetry_node) {
                return Some(name);
            }
        }
    }

    // Top-level title = "..." (Hugo / site config)
    if let Some(title) = taplo_str_value(&root.get("title")) {
        if !title.is_empty() && title.len() <= 60 {
            return Some(title);
        }
    }

    None
}

/// TOML: taplo-based AST extraction with known-pattern detection and noise
/// section filtering. Falls back to regex on parse failure.
pub(super) fn extract_toml(content: &str) -> Option<String> {
    let parse_result = taplo::parser::parse(content);
    let dom = parse_result.into_dom();

    // If taplo produced an invalid root, fall back to regex.
    if dom.is_invalid() {
        return extract_toml_regex_fallback(content);
    }

    // 1) Known pattern detection.
    if let Some(stem) = toml_detect_known_pattern(&dom) {
        return Some(stem);
    }

    // 2) General extraction: collect section headers + values.
    let root_table = match dom.as_table() {
        Some(t) => t,
        None => return extract_toml_regex_fallback(content),
    };

    let entries = root_table.entries().read();
    let mut tokens: Vec<String> = Vec::new();
    for (key, node) in entries.iter() {
        if tokens.len() >= MAX_TOKENS {
            break;
        }
        let key_str = key.value().to_string();
        if is_toml_noise_section(&key_str) {
            continue;
        }
        // For table sections that have a `name` key, use the name value.
        if node.is_table() {
            if let Some(name) = taplo_table_name_value(node) {
                tokens.push(name);
                continue;
            }
        }
        tokens.push(key_str);
    }

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join("-"))
    }
}

/// Regex-based TOML fallback (used when taplo fails to parse).
fn extract_toml_regex_fallback(content: &str) -> Option<String> {
    let section_re = regex::Regex::new(r"^\[([a-zA-Z_][a-zA-Z0-9_\-\.]*)\]").ok()?;
    let key_re = regex::Regex::new(r"^([a-zA-Z_][a-zA-Z0-9_\-]*)[\s]*=").ok()?;

    let mut tokens: Vec<String> = Vec::new();
    for line in content.lines() {
        if tokens.len() >= MAX_TOKENS {
            break;
        }
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some(cap) = section_re.captures(t).and_then(|c| c.get(1)) {
            tokens.push(cap.as_str().to_string());
        } else if let Some(cap) = key_re.captures(t).and_then(|c| c.get(1)) {
            tokens.push(cap.as_str().to_string());
        }
    }

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join("-"))
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----- CSV tests -------------------------------------------------------

    #[test]
    fn csv_basic_semantic_headers() {
        let csv = "name,email,department,salary\nAlice,a@b.com,Eng,100000";
        assert_eq!(extract_csv(csv).unwrap(), "name-email-department-salary");
    }

    #[test]
    fn csv_filters_id_and_timestamp_columns() {
        let csv = "id,name,email,created_at,updated_at\n1,Alice,a@b.com,2024-01-01,2024-06-01";
        assert_eq!(extract_csv(csv).unwrap(), "name-email");
    }

    #[test]
    fn csv_filters_sales_date_columns() {
        let csv = "date,product,region,quantity,revenue\n2024-01-15,Widget,North,100,5000";
        assert_eq!(extract_csv(csv).unwrap(), "product-region-quantity-revenue");
    }

    #[test]
    fn csv_filters_log_timestamp_and_uuid() {
        let csv = "timestamp,uuid,level,message,source\n2024-01-15T10:30:00Z,abc-123,ERROR,Disk full,server-1";
        assert_eq!(extract_csv(csv).unwrap(), "level-message-source");
    }

    #[test]
    fn csv_filters_coordinate_columns() {
        let csv = "city,latitude,longitude,population\nNYC,40.71,-74.01,8000000";
        assert_eq!(extract_csv(csv).unwrap(), "city-population");
    }

    #[test]
    fn csv_filters_generic_positional_columns() {
        let csv = "col1,col2,col3,col4\n1,2,3,4";
        // All columns are noise → None.
        assert!(extract_csv(csv).is_none());
    }

    #[test]
    fn csv_filters_unnamed_columns() {
        let csv = "unnamed_0,unnamed_1,category,value\n0,1,A,42";
        assert_eq!(extract_csv(csv).unwrap(), "category-value");
    }

    #[test]
    fn csv_filters_foreign_key_id_suffix() {
        let csv = "employee_id,order_id,product_name,quantity\n1,100,Widget,5";
        assert_eq!(extract_csv(csv).unwrap(), "product_name-quantity");
    }

    #[test]
    fn csv_respects_max_tokens() {
        let csv = "name,email,department,salary,location,manager\nA,a@b,Eng,100,NYC,Bob";
        let result = extract_csv(csv).unwrap();
        // MAX_TOKENS is 4, so we get first 4 semantic columns.
        assert_eq!(result, "name-email-department-salary");
    }

    #[test]
    fn csv_tab_delimited_with_noise() {
        let csv = "id\tname\temail\ttimestamp\n1\tAlice\ta@b\t2024-01-01";
        assert_eq!(extract_csv(csv).unwrap(), "name-email");
    }

    #[test]
    fn csv_all_noise_returns_none() {
        let csv = "id,uuid,created_at,updated_at\n1,abc,2024-01-01,2024-06-01";
        assert!(extract_csv(csv).is_none());
    }

    #[test]
    fn csv_empty_returns_none() {
        assert!(extract_csv("").is_none());
        assert!(extract_csv("   ").is_none());
    }

    // ----- JSON tests ------------------------------------------------------

    #[test]
    fn json_package_json_extracts_name_value() {
        let json = r#"{
            "name": "my-cool-app",
            "version": "1.0.0",
            "scripts": { "build": "tsc" },
            "dependencies": { "express": "^4.0" }
        }"#;
        assert_eq!(extract_json(json).unwrap(), "my-cool-app");
    }

    #[test]
    fn json_openapi_extracts_info_title() {
        let json = r#"{
            "openapi": "3.0.0",
            "info": { "title": "Pet Store API", "version": "1.0" },
            "paths": {}
        }"#;
        assert_eq!(extract_json(json).unwrap(), "Pet Store API");
    }

    #[test]
    fn json_swagger_extracts_info_title() {
        let json = r#"{
            "swagger": "2.0",
            "info": { "title": "Legacy API", "version": "1.0" },
            "paths": {}
        }"#;
        assert_eq!(extract_json(json).unwrap(), "Legacy API");
    }

    #[test]
    fn json_tsconfig_detected() {
        let json = r#"{
            "compilerOptions": { "target": "es2020", "strict": true },
            "include": ["src"]
        }"#;
        assert_eq!(extract_json(json).unwrap(), "tsconfig");
    }

    #[test]
    fn json_geojson_feature_collection() {
        let json = r#"{
            "type": "FeatureCollection",
            "features": []
        }"#;
        assert_eq!(extract_json(json).unwrap(), "feature-collection");
    }

    #[test]
    fn json_schema_with_title() {
        let json = r#"{
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "User Profile",
            "type": "object",
            "properties": {}
        }"#;
        assert_eq!(extract_json(json).unwrap(), "User Profile");
    }

    #[test]
    fn json_error_response_extracts_error_value() {
        let json = r#"{
            "error": "Not Found",
            "status": 404,
            "path": "/api/users/999"
        }"#;
        assert_eq!(extract_json(json).unwrap(), "Not Found-status-path");
    }

    #[test]
    fn json_name_value_extraction() {
        let json = r#"{
            "name": "grayslate",
            "description": "A developer scratchpad"
        }"#;
        assert_eq!(extract_json(json).unwrap(), "grayslate-description");
    }

    #[test]
    fn json_filters_noise_keys() {
        let json = r#"{
            "id": 42,
            "_id": "abc123",
            "username": "alice",
            "email": "alice@example.com",
            "created_at": "2024-01-01",
            "updated_at": "2024-06-01"
        }"#;
        assert_eq!(extract_json(json).unwrap(), "username-email");
    }

    #[test]
    fn json_type_key_noise_only_for_generic_values() {
        // `type` with a generic value → noise.
        let json = r#"{ "type": "object", "name": "Foo" }"#;
        assert_eq!(extract_json(json).unwrap(), "Foo");

        // `type` with a specific value → kept.
        let json2 = r#"{ "type": "user", "name": "Alice" }"#;
        let result = extract_json(json2).unwrap();
        assert!(result.contains("type"));
    }

    #[test]
    fn json_array_of_objects() {
        let json = r#"[
            { "city": "NYC", "population": 8000000, "country": "US" },
            { "city": "London", "population": 9000000, "country": "UK" }
        ]"#;
        assert_eq!(extract_json(json).unwrap(), "city-population-country");
    }

    #[test]
    fn json_partial_parse_regex_fallback() {
        let json = r#"{ "name": "test", "version": "1.0", broken..."#;
        let result = extract_json(json).unwrap();
        assert!(result.contains("name"));
    }

    #[test]
    fn json_empty_returns_none() {
        assert!(extract_json("").is_none());
        assert!(extract_json("   ").is_none());
    }

    // ----- TOML tests ------------------------------------------------------

    #[test]
    fn toml_cargo_toml_extracts_package_name() {
        let toml = r#"
[package]
name = "my-rust-app"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1"
"#;
        assert_eq!(extract_toml(toml).unwrap(), "my-rust-app");
    }

    #[test]
    fn toml_pyproject_extracts_project_name() {
        let toml = r#"
[project]
name = "my-python-lib"
version = "2.0.0"
description = "A Python library"

[build-system]
requires = ["setuptools"]
"#;
        assert_eq!(extract_toml(toml).unwrap(), "my-python-lib");
    }

    #[test]
    fn toml_poetry_extracts_name() {
        let toml = r#"
[tool.poetry]
name = "poetry-project"
version = "1.0.0"
description = "A Poetry project"

[tool.poetry.dependencies]
python = "^3.9"
"#;
        assert_eq!(extract_toml(toml).unwrap(), "poetry-project");
    }

    #[test]
    fn toml_hugo_config_extracts_title() {
        let toml = r#"
title = "My Hugo Site"
baseURL = "https://example.com"
languageCode = "en-us"
theme = "ananke"
"#;
        assert_eq!(extract_toml(toml).unwrap(), "My Hugo Site");
    }

    #[test]
    fn toml_server_config_general_extraction() {
        let toml = r#"
[server]
host = "0.0.0.0"
port = 8080

[database]
url = "postgres://localhost/mydb"

[logging]
level = "info"
"#;
        let result = extract_toml(toml).unwrap();
        assert_eq!(result, "server-database-logging");
    }

    #[test]
    fn toml_skips_noise_sections() {
        let toml = r#"
[package]
name = "noisy-crate"
version = "0.1.0"

[dependencies]
serde = "1"

[build-dependencies]
cc = "1"

[profile.release]
opt-level = 3

[target.x86_64-unknown-linux-gnu]
linker = "clang"
"#;
        // package name detected → known pattern.
        assert_eq!(extract_toml(toml).unwrap(), "noisy-crate");
    }

    #[test]
    fn toml_workspace_config() {
        let toml = r#"
[workspace]
members = ["crate-a", "crate-b"]

[workspace.dependencies]
serde = "1"
"#;
        let result = extract_toml(toml).unwrap();
        assert_eq!(result, "workspace");
    }

    #[test]
    fn toml_section_with_name_value() {
        let toml = r#"
[database]
name = "production_db"
host = "localhost"

[cache]
name = "redis_cache"
ttl = 300
"#;
        let result = extract_toml(toml).unwrap();
        assert_eq!(result, "production_db-redis_cache");
    }

    #[test]
    fn toml_regex_fallback_for_invalid_toml() {
        // Intentionally broken TOML that taplo cannot parse.
        let toml = "[section]\nkey = \n[other]\nfoo = ";
        let result = extract_toml(toml);
        // Regex fallback should still extract something.
        assert!(result.is_some());
    }

    #[test]
    fn toml_empty_returns_none() {
        assert!(extract_toml("").is_none());
        assert!(extract_toml("   ").is_none());
    }

    #[test]
    fn toml_simple_key_value_no_sections() {
        let toml = r#"
name = "simple"
version = "1.0"
debug = true
"#;
        // No known pattern (no [package]/[project]), top-level `name` not detected
        // by known pattern, but general extraction collects keys.
        // However, top-level title detection won't match here since it's `name` not `title`.
        let result = extract_toml(toml).unwrap();
        assert!(result.contains("name") || result.contains("simple"));
    }
}
