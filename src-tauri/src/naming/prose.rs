// ---------------------------------------------------------------------------
// YAKE extractor (prose / plain text)
// ---------------------------------------------------------------------------

pub(super) fn extract_yake(content: &str) -> Option<String> {
    use yake_rust::{get_n_best, Config, StopWords};

    if content.trim().is_empty() {
        return None;
    }

    // `predefined` returns Option, not Result, in 1.x.
    let stop_words = StopWords::predefined("en")?;
    let config = Config {
        ngrams: 2,
        ..Config::default()
    };

    let keywords = get_n_best(4, content, &stop_words, &config);
    if keywords.is_empty() {
        return None;
    }

    // ResultItem.raw is the original-cased phrase in 1.x; use it directly.
    let stems: Vec<&str> = keywords
        .iter()
        .take(3)
        .map(|item| item.raw.as_str())
        .collect();
    if stems.is_empty() {
        None
    } else {
        Some(stems.join("-"))
    }
}
