/// name_file — thin CLI wrapper around grayslate_lib naming + detection pipelines.
///
/// Usage:
///   echo "file content" | name_file
///
/// Reads document content from stdin and performs content-only analysis:
///   1. Detects the language without any filename hint (mirrors paste/untitled flow).
///   2. Runs the naming pipeline using the detected language.
///
/// Writes a single JSON object to stdout:
///
///   {
///     "content_detected_lang": "python",   // language detected from content alone
///     "content_suggested_ext": "py",        // canonical extension for that language
///     "suggested_name":        "my-module" // naming result, or "" on fallback
///   }
///
/// This binary is intended for use by audit_repos.py to evaluate how well the
/// Grayslate detection and naming pipelines handle content without any filename
/// context — the primary path for paste/untitled documents.
use std::io::{self, Read};

use grayslate_lib::{detection, naming};

fn main() {
    let mut content = String::new();
    io::stdin().read_to_string(&mut content).ok();

    // Content-only detection: no filename hint, mirrors the paste/untitled path.
    let detected_lang = detection::detect_language(&content, None)
        .unwrap_or("text")
        .to_string();

    let suggested_ext = naming::language_to_extension(&detected_lang);

    // Naming uses the detected language; no filename context.
    let suggested_name = naming::suggest_stem(&content, &detected_lang)
        .unwrap_or_default();

    let json = format!(
        "{{\"content_detected_lang\":{},\"content_suggested_ext\":{},\"suggested_name\":{}}}",
        json_str(&detected_lang),
        json_str(suggested_ext),
        json_str(&suggested_name),
    );
    println!("{}", json);
}

/// Minimal JSON string escaping for the fields we emit (no control chars expected).
fn json_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
