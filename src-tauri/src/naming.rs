/// Thin re-export of the `grayslate-langnaming` crate plus the
/// `suggest_stem_auto` glue function that combines naming + detection.
pub use grayslate_langnaming::*;

/// Auto-detecting variant of `suggest_stem`.
///
/// When `language_hint` is empty or `"auto"`, runs the detection pipeline
/// to identify the language from content (using `filename` as a hint for
/// extension-based Phase 1 detection). Returns the stem alongside the
/// effective language that was used (so the caller can propagate it).
pub fn suggest_stem_auto(
    content: &str,
    language_hint: &str,
    filename: Option<&str>,
) -> (Option<String>, String) {
    let effective = if language_hint.is_empty() || language_hint == "auto" {
        crate::detection::detect_language(content, filename)
            .unwrap_or("text")
            .to_string()
    } else {
        language_hint.to_string()
    };
    (
        grayslate_langnaming::suggest_stem(content, &effective),
        effective,
    )
}
