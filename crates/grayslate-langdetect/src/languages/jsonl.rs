use super::json::is_balanced_json;
use super::{ContentFamily, LanguageDefinition};

/// Detect high-confidence JSON Lines content by checking a bounded sample of
/// non-empty records. Object/array records are intentionally required here:
/// scalar-only content is too ambiguous to classify without a filename.
pub(crate) fn is_likely_jsonl(trimmed: &str, was_sliced: bool) -> bool {
    let mut records: Vec<&str> = trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    // The shared byte bound preserves UTF-8 boundaries, but it can still cut
    // through a JSONL record. In that case the final line is not trustworthy.
    if was_sliced {
        records.pop();
    }

    if records.len() < 2 {
        return false;
    }

    records
        .iter()
        .take(5)
        .copied()
        .all(is_balanced_json)
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "jsonl",
        extensions: &[".jsonl", ".ndjson"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(4),
        structural_detect: Some(is_likely_jsonl),
        keywords: &[],
        builtins: &[],
        content_families: &[ContentFamily::StructuredData],
        anchors: &[],
        hints: &[],
        disqualifiers: &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_object_records() {
        let content = "{\"name\":\"Alice\"}\n{\"name\":\"Bob\"}\n";
        assert!(is_likely_jsonl(content, false));
    }

    #[test]
    fn rejects_single_json_value() {
        assert!(!is_likely_jsonl("{\"name\":\"Alice\"}", false));
    }

    #[test]
    fn rejects_non_json_record() {
        let content = "{\"name\":\"Alice\"}\nnot json\n";
        assert!(!is_likely_jsonl(content, false));
    }

    #[test]
    fn ignores_truncated_final_record_from_bounded_content() {
        let content = "{\"name\":\"Alice\"}\n{\"name\":\"Bob\"}\n{\"payload\":\"partial";
        assert!(is_likely_jsonl(content, true));
        assert!(!is_likely_jsonl(content, false));
    }
}
