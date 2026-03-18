use super::model::MAX_TOKENS;

// ---------------------------------------------------------------------------
// Per-format extractors
// ---------------------------------------------------------------------------

/// CSV: take first 3–4 non-empty header column names from the first line.
pub(super) fn extract_csv(content: &str) -> Option<String> {
    let first_line = content.lines().next()?.trim();
    if first_line.is_empty() {
        return None;
    }

    // Detect delimiter (tab wins if more tabs than commas).
    let delimiter = if first_line.matches('\t').count() > first_line.matches(',').count() {
        '\t'
    } else {
        ','
    };

    let headers: Vec<&str> = first_line
        .split(delimiter)
        .map(|h| h.trim().trim_matches('"').trim_matches('\'').trim())
        .filter(|h| !h.is_empty())
        .take(MAX_TOKENS)
        .collect();

    if headers.is_empty() {
        return None;
    }
    Some(headers.join("-"))
}

/// JSON: extract first few top-level keys (object) or keys from first array
/// element (array-of-objects).
pub(super) fn extract_json(content: &str) -> Option<String> {
    // Quick structural check.
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Try parsing the bounded content; fall back to partial-parse regex.
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return json_keys_from_value(&value);
    }

    // Partial parse via regex: find `"key":` patterns near the start.
    let re = regex::Regex::new(r#""([a-zA-Z_$][a-zA-Z0-9_$]*?)"\s*:"#).ok()?;
    let keys: Vec<String> = re
        .captures_iter(trimmed)
        .take(MAX_TOKENS)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect();

    if keys.is_empty() {
        None
    } else {
        Some(keys.join("-"))
    }
}

fn json_keys_from_value(value: &serde_json::Value) -> Option<String> {
    let obj = match value {
        serde_json::Value::Object(map) => Some(map),
        serde_json::Value::Array(arr) => arr.iter().find_map(|v| v.as_object()),
        _ => None,
    }?;

    let keys: Vec<&str> = obj.keys().take(MAX_TOKENS).map(|s| s.as_str()).collect();
    if keys.is_empty() {
        None
    } else {
        Some(keys.join("-"))
    }
}

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

/// TOML: extract first few `key =` lines or `[section]` headers.
pub(super) fn extract_toml(content: &str) -> Option<String> {
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
