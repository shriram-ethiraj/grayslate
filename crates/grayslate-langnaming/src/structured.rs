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
/// Falls back to row-count + column-count description when all headers are
/// generic/noise (e.g., "col1,col2,col3" → "3-cols-5-rows").
pub(crate) fn extract_csv(content: &str) -> Option<String> {
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

    if !semantic.is_empty() {
        let tokens: Vec<&str> = semantic.into_iter().take(MAX_TOKENS).collect();
        return Some(tokens.join("-"));
    }

    // All headers are generic — produce a descriptive shape stem
    let col_count = all_headers.len();
    let row_count = content.lines().count().saturating_sub(1); // exclude header
    if row_count > 0 {
        Some(format!("{col_count}-cols-{row_count}-rows"))
    } else {
        Some(format!("{col_count}-cols"))
    }
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

fn is_json_noise_key(key: &str, value: Option<&str>) -> bool {
    let lower = key.to_lowercase();
    if JSON_NOISE_KEYS.contains(&lower.as_str()) {
        return true;
    }
    if lower == "type" {
        if let Some(val) = value {
            if JSON_GENERIC_TYPE_VALUES.contains(&val.to_lowercase().as_str()) {
                return true;
            }
        }
    }
    false
}

/// Truncate a string value at a word boundary within 60 chars.
fn json_truncate_value(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.len() <= 60 {
        return Some(trimmed.to_string());
    }
    let cut = &trimmed[..60];
    let end = cut.rfind(' ').unwrap_or(60);
    if end > 10 {
        Some(cut[..end].to_string())
    } else {
        Some(cut.to_string())
    }
}

/// Lightweight top-level key-value scanner for JSON.
///
/// Walks the content character-by-character, tracking brace/bracket depth.
/// Collects `"key": "value"` pairs at depth 1 (root object) or depth 2
/// (first object inside a root array).
fn json_scan_top_level(content: &str) -> Vec<(String, Option<String>)> {
    let bytes = content.trim().as_bytes();
    if bytes.is_empty() {
        return Vec::new();
    }

    let first = bytes[0];
    if first != b'{' && first != b'[' {
        return Vec::new();
    }
    // For `[{...}]` arrays, keys are at depth 2; for `{...}` objects, depth 1.
    let target_depth: i32 = if first == b'[' { 2 } else { 1 };

    let mut pairs: Vec<(String, Option<String>)> = Vec::new();
    let mut depth: i32 = 0;
    let mut i = 0;
    let len = bytes.len();

    while i < len && pairs.len() < 40 {
        match bytes[i] {
            b'"' => {
                let start = i + 1;
                i += 1;
                while i < len && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                let end = i.min(len);
                i += 1; // skip closing "

                // Check if followed by `:` (this string is a key)
                let mut j = i;
                while j < len && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < len && bytes[j] == b':' && depth == target_depth {
                    let key = String::from_utf8_lossy(&bytes[start..end]).to_string();
                    j += 1; // skip ':'
                    while j < len && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    // Try to extract a string value
                    let value = if j < len && bytes[j] == b'"' {
                        let vstart = j + 1;
                        let mut vend = vstart;
                        while vend < len && bytes[vend] != b'"' {
                            if bytes[vend] == b'\\' {
                                vend += 1;
                            }
                            vend += 1;
                        }
                        Some(
                            String::from_utf8_lossy(&bytes[vstart..vend.min(len)])
                                .to_string(),
                        )
                    } else {
                        None
                    };
                    pairs.push((key, value));
                }
            }
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                depth -= 1;
                // For arrays, stop after the first object closes back to depth 1
                if first == b'[' && depth < target_depth - 1 {
                    break;
                }
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    pairs
}

/// Known-pattern detection using scanned key-value pairs.
fn json_detect_known_pattern(pairs: &[(String, Option<String>)]) -> Option<String> {
    let has = |k: &str| pairs.iter().any(|(key, _)| key == k);
    let str_val = |k: &str| {
        pairs
            .iter()
            .find(|(key, _)| key == k)
            .and_then(|(_, v)| v.as_deref())
    };

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
        if let Some(schema_url) = str_val("$schema") {
            if schema_url.contains("json-schema.org") {
                return Some("json-schema".to_string());
            }
        }
    }

    // OpenAPI / Swagger: nested info.title requires raw content — handled in extract_json.

    // tsconfig:{compilerOptions}
    if has("compilerOptions") {
        return Some("tsconfig".to_string());
    }

    // GeoJSON: {type: "FeatureCollection" | "Feature"}
    if let Some(type_val) = str_val("type") {
        if type_val == "FeatureCollection" || type_val == "Feature" {
            return Some("geojson".to_string());
        }
    }

    // ESLint config: {rules} with {extends|plugins|env|parserOptions}
    if has("rules") && (has("extends") || has("plugins") || has("env") || has("parserOptions")) {
        return Some("eslint-config".to_string());
    }

    // Babel config: {presets} or {plugins} with one of {env, sourceType, targets}
    if (has("presets") || has("plugins"))
        && (has("env") || has("sourceType") || has("targets"))
    {
        return Some("babel-config".to_string());
    }

    // Prettier config: {trailingComma|singleQuote|tabWidth|printWidth}
    if (has("trailingComma") || has("singleQuote") || has("semi"))
        && (has("tabWidth") || has("printWidth"))
    {
        return Some("prettier-config".to_string());
    }

    // VS Code settings: {editor.*|workbench.*} keys
    if pairs
        .iter()
        .any(|(k, _)| k.starts_with("editor.") || k.starts_with("workbench.") || k.starts_with("files."))
    {
        return Some("vscode-settings".to_string());
    }

    // VS Code launch.json: {version, configurations}
    if has("version") && has("configurations") {
        return Some("vscode-launch".to_string());
    }

    // VS Code tasks.json: {version, tasks}
    if has("version") && has("tasks") && !has("scripts") {
        return Some("vscode-tasks".to_string());
    }

    // Lerna: {packages, npmClient|useWorkspaces}
    if has("packages") && (has("npmClient") || has("useWorkspaces")) {
        return Some("lerna-config".to_string());
    }

    // Nx workspace: {projects} with {npmScope|affected}
    if has("projects") && (has("npmScope") || has("affected")) {
        return Some("nx-workspace".to_string());
    }

    // Docker container inspect: {Id, Created, State, Config}
    if has("Id") && has("Created") && has("State") && has("Config") {
        if let Some(name) = str_val("Name") {
            let name = name.trim_start_matches('/');
            if !name.is_empty() {
                return Some(format!("container-{name}"));
            }
        }
        return Some("docker-container".to_string());
    }

    // Composer (PHP): {require} with {name, autoload|scripts}
    if has("require") && has("name") && (has("autoload") || has("scripts")) {
        if let Some(name) = str_val("name") {
            if !name.is_empty() && name.len() <= 60 {
                return Some(name.to_string());
            }
        }
    }

    None
}

/// JSON: pure regex/scanner extraction — no serde_json.
pub(crate) fn extract_json(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }

    let pairs = json_scan_top_level(trimmed);
    if pairs.is_empty() {
        // Degenerate / broken JSON — regex key fallback
        static KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r#""([a-zA-Z_$][a-zA-Z0-9_$]*?)"\s*:"#).unwrap()
        });
        let keys: Vec<String> = KEY_RE
            .captures_iter(trimmed)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .filter(|k| !is_json_noise_key(k, None))
            .take(MAX_TOKENS)
            .collect();
        return if keys.is_empty() {
            None
        } else {
            Some(keys.join("-"))
        };
    }

    // 1) Known pattern detection.
    if let Some(stem) = json_detect_known_pattern(&pairs) {
        return Some(stem);
    }

    // 1b) OpenAPI/Swagger nested info.title — uses regex on raw content.
    let has = |k: &str| pairs.iter().any(|(key, _)| key == k);
    if has("swagger") || has("openapi") {
        static INFO_TITLE_RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r#""info"\s*:\s*\{[^}]*?"title"\s*:\s*"([^"]{1,60})""#).unwrap()
        });
        if let Some(cap) = INFO_TITLE_RE.captures(trimmed) {
            let title = cap[1].trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }

    // 2) Collect non-noise keys with semantic value extraction.
    let mut tokens: Vec<String> = Vec::new();
    for (key, val) in &pairs {
        if tokens.len() >= MAX_TOKENS {
            break;
        }
        if is_json_noise_key(key, val.as_deref()) {
            continue;
        }
        // For `name`, `title`, `error`: prefer the VALUE over the key name.
        let lower = key.to_lowercase();
        if matches!(lower.as_str(), "name" | "title" | "error") {
            if let Some(ref v) = val {
                if let Some(truncated) = json_truncate_value(v) {
                    tokens.push(truncated);
                    continue;
                }
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
/// Kept as shared utility for tests and backward compatibility.
#[allow(dead_code)]
pub(crate) fn extract_yaml(content: &str) -> Option<String> {
    // Keys whose *value* is a better filename than the key itself.
    const SEMANTIC_KEYS: &[&str] = &[
        "name", "title", "description", "summary", "label", "id",
        "apiVersion", "kind",
    ];

    // Matches `key: value` at any indentation level (for semantic key scan).
    let re_kv = regex::Regex::new(
        r#"^\s*([a-zA-Z_][a-zA-Z0-9_\-]*)[\s]*:[\s]+["']?([^"'\n\r]+?)["']?[\s]*$"#,
    ).ok()?;
    // Matches top-level `key:` only (no leading whitespace).
    let re_key = regex::Regex::new(r"^([a-zA-Z_][a-zA-Z0-9_\-]*)[\s]*:").ok()?;

    let meaningful_lines = content.lines().filter(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with('#') && !t.starts_with("---") && !t.starts_with("...")
    });

    // First pass: look for a semantic key whose value makes a good name.
    // Scans all indentation levels so `metadata:\n  name: my-app` is caught.
    for line in meaningful_lines.clone() {
        if let Some(caps) = re_kv.captures(line) {
            let key = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let val = caps.get(2).map(|m| m.as_str()).unwrap_or("").trim();
            if SEMANTIC_KEYS.iter().any(|sk| sk.eq_ignore_ascii_case(key))
                && !val.is_empty()
                && val.len() <= 80
            {
                return Some(val.to_string());
            }
        }
    }

    // Fallback: use the first top-level key name.
    let keys: Vec<String> = content
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#') && !t.starts_with("---") && !t.starts_with("...")
        })
        .filter(|l| !l.starts_with(char::is_whitespace)) // top-level only
        .filter_map(|l| {
            re_key
                .captures(l)
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

/// TOML: pure regex-based extraction with known-pattern detection and noise
/// section filtering.
pub(crate) fn extract_toml(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }

    static SECTION_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^\[([a-zA-Z_][\w\-\.]*)\]\s*$").unwrap()
    });
    static NAME_VALUE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"^name\s*=\s*["']([^"']{1,60})["']"#).unwrap()
    });
    static STR_VALUE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"^([\w\-]+)\s*=\s*["']([^"']{1,60})["']"#).unwrap()
    });
    static KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^([\w\-]+)\s*=").unwrap()
    });

    // --- Scan the file structure ---
    struct SectionInfo {
        header: String,
        name_value: Option<String>,
    }
    let mut sections: Vec<SectionInfo> = Vec::new();
    let mut top_keys: Vec<String> = Vec::new();
    let mut top_str_values: Vec<(String, String)> = Vec::new();
    let mut current_section: Option<usize> = None;

    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some(cap) = SECTION_RE.captures(t) {
            sections.push(SectionInfo {
                header: cap[1].to_string(),
                name_value: None,
            });
            current_section = Some(sections.len() - 1);
            continue;
        }
        if let Some(idx) = current_section {
            if sections[idx].name_value.is_none() {
                if let Some(cap) = NAME_VALUE_RE.captures(t) {
                    sections[idx].name_value = Some(cap[1].to_string());
                }
            }
        } else {
            // Top-level key (before any section)
            if let Some(cap) = STR_VALUE_RE.captures(t) {
                let key = cap[1].to_string();
                let val = cap[2].to_string();
                top_str_values.push((key.clone(), val));
                top_keys.push(key);
            } else if let Some(cap) = KEY_RE.captures(t) {
                top_keys.push(cap[1].to_string());
            }
        }
    }

    let has_section =
        |s: &str| sections.iter().any(|si| si.header.eq_ignore_ascii_case(s));
    let has_top_key = |k: &str| top_keys.iter().any(|tk| tk == k);
    let section_name = |s: &str| {
        sections
            .iter()
            .find(|si| si.header.eq_ignore_ascii_case(s))
            .and_then(|si| si.name_value.clone())
    };

    // --- Known Pattern Detection ---

    // Cargo.toml: [package] with name
    if let Some(name) = section_name("package") {
        return Some(name);
    }
    // pyproject.toml: [project] with name
    if let Some(name) = section_name("project") {
        return Some(name);
    }
    // Poetry: [tool.poetry] with name
    if let Some(name) = section_name("tool.poetry") {
        return Some(name);
    }
    // Tool configs
    if has_section("tool.ruff") {
        return Some("ruff-config".to_string());
    }
    if has_section("tool.black") {
        return Some("black-config".to_string());
    }
    if has_section("tool.mypy") {
        return Some("mypy-config".to_string());
    }
    // rustfmt.toml
    if has_top_key("max_width") || (has_top_key("edition") && has_top_key("hard_tabs")) {
        return Some("rustfmt".to_string());
    }
    // clippy.toml
    if has_top_key("cognitive-complexity-threshold")
        || has_top_key("too-many-arguments-threshold")
    {
        return Some("clippy-config".to_string());
    }
    // .cargo/config.toml
    if has_section("build") && (has_section("registries") || has_section("source")) {
        return Some("cargo-config".to_string());
    }
    // Top-level title = "..." (Hugo / site config)
    if let Some((_, val)) = top_str_values.iter().find(|(k, _)| k == "title") {
        if !val.is_empty() {
            return Some(val.clone());
        }
    }

    // --- General Extraction ---
    let mut tokens: Vec<String> = Vec::new();
    for si in &sections {
        if tokens.len() >= MAX_TOKENS {
            break;
        }
        if is_toml_noise_section(&si.header) {
            continue;
        }
        if let Some(ref name) = si.name_value {
            tokens.push(name.clone());
        } else {
            tokens.push(si.header.clone());
        }
    }
    // Fall back to top-level keys if no sections provided tokens
    if tokens.is_empty() {
        for key in &top_keys {
            if tokens.len() >= MAX_TOKENS {
                break;
            }
            tokens.push(key.clone());
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
        assert_eq!(extract_csv(csv).unwrap(), "name");
    }

    #[test]
    fn csv_filters_id_and_timestamp_columns() {
        let csv = "id,name,email,created_at,updated_at\n1,Alice,a@b.com,2024-01-01,2024-06-01";
        assert_eq!(extract_csv(csv).unwrap(), "name");
    }

    #[test]
    fn csv_filters_sales_date_columns() {
        let csv = "date,product,region,quantity,revenue\n2024-01-15,Widget,North,100,5000";
        assert_eq!(extract_csv(csv).unwrap(), "product");
    }

    #[test]
    fn csv_filters_log_timestamp_and_uuid() {
        let csv = "timestamp,uuid,level,message,source\n2024-01-15T10:30:00Z,abc-123,ERROR,Disk full,server-1";
        assert_eq!(extract_csv(csv).unwrap(), "level");
    }

    #[test]
    fn csv_filters_coordinate_columns() {
        let csv = "city,latitude,longitude,population\nNYC,40.71,-74.01,8000000";
        assert_eq!(extract_csv(csv).unwrap(), "city");
    }

    #[test]
    fn csv_filters_generic_positional_columns() {
        let csv = "col1,col2,col3,col4\n1,2,3,4";
        // All columns are noise → shape-based fallback
        let result = extract_csv(csv).unwrap();
        assert!(result.contains("4-cols"), "shape fallback: {result}");
    }

    #[test]
    fn csv_filters_unnamed_columns() {
        let csv = "unnamed_0,unnamed_1,category,value\n0,1,A,42";
        assert_eq!(extract_csv(csv).unwrap(), "category");
    }

    #[test]
    fn csv_filters_foreign_key_id_suffix() {
        let csv = "employee_id,order_id,product_name,quantity\n1,100,Widget,5";
        assert_eq!(extract_csv(csv).unwrap(), "product_name");
    }

    #[test]
    fn csv_respects_max_tokens_single() {
        let csv = "name,email,department,salary,location,manager\nA,a@b,Eng,100,NYC,Bob";
        let result = extract_csv(csv).unwrap();
        // MAX_TOKENS is 1, so we get only the first semantic column.
        assert_eq!(result, "name");
    }

    #[test]
    fn csv_tab_delimited_with_noise() {
        let csv = "id\tname\temail\ttimestamp\n1\tAlice\ta@b\t2024-01-01";
        assert_eq!(extract_csv(csv).unwrap(), "name");
    }

    #[test]
    fn csv_all_noise_returns_shape() {
        let csv = "id,uuid,created_at,updated_at\n1,abc,2024-01-01,2024-06-01";
        let result = extract_csv(csv).unwrap();
        assert!(result.contains("4-cols") && result.contains("1-rows"), "shape: {result}");
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
        assert_eq!(extract_json(json).unwrap(), "geojson");
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
        assert_eq!(extract_json(json).unwrap(), "Not Found");
    }

    #[test]
    fn json_name_value_extraction() {
        let json = r#"{
            "name": "grayslate",
            "description": "A developer scratchpad"
        }"#;
        assert_eq!(extract_json(json).unwrap(), "grayslate");
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
        assert_eq!(extract_json(json).unwrap(), "username");
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
        assert_eq!(extract_json(json).unwrap(), "city");
    }

    #[test]
    fn json_partial_parse_regex_fallback() {
        let json = r#"{ "name": "test", "version": "1.0", broken..."#;
        let result = extract_json(json).unwrap();
        // Scanner extracts the "name" value ("test") even from broken JSON.
        assert!(result.contains("test"), "got: {result}");
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
        assert_eq!(result, "server");
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
        assert_eq!(result, "production_db");
    }

    #[test]
    fn toml_regex_fallback_for_invalid_toml() {
        // Intentionally broken TOML (missing values after `=`).
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

    // ----- YAML tests ------------------------------------------------------

    #[test]
    fn yaml_name_value_preferred() {
        let yaml = "---\nname: my-service\nversion: 1.0\n";
        assert_eq!(extract_yaml(yaml).unwrap(), "my-service");
    }

    #[test]
    fn yaml_title_value_preferred() {
        let yaml = "title: Weekly Status Report\nauthor: alice\n";
        assert_eq!(extract_yaml(yaml).unwrap(), "Weekly Status Report");
    }

    #[test]
    fn yaml_falls_back_to_first_key() {
        let yaml = "bugfixes:\n  - Fixed some issue\nminor_changes:\n  - Updated docs\n";
        assert_eq!(extract_yaml(yaml).unwrap(), "bugfixes");
    }

    #[test]
    fn yaml_skips_comments_and_markers() {
        let yaml = "---\n# This is a comment\nname: app-config\nport: 8080\n";
        assert_eq!(extract_yaml(yaml).unwrap(), "app-config");
    }

    #[test]
    fn yaml_api_version_kind() {
        let yaml = "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\n";
        assert_eq!(extract_yaml(yaml).unwrap(), "apps/v1");
    }

    #[test]
    fn yaml_no_semantic_key_uses_first_top_level() {
        let yaml = "servers:\n  - host: localhost\n    port: 3000\n";
        assert_eq!(extract_yaml(yaml).unwrap(), "servers");
    }

    #[test]
    fn yaml_empty_returns_none() {
        assert!(extract_yaml("").is_none());
        assert!(extract_yaml("   \n\n").is_none());
    }

    #[test]
    fn yaml_indented_name_not_top_level_key() {
        // Only top-level keys should be used as fallback, not nested ones.
        let yaml = "metadata:\n  name: nested-app\n  labels:\n    app: web\n";
        // "name" on indented line is still a semantic key and should be found.
        assert_eq!(extract_yaml(yaml).unwrap(), "nested-app");
    }
}
