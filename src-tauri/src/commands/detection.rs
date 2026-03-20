/// detection.rs (commands)
///
/// Tauri commands for language detection.
/// Exposes the detection module's pipeline to the frontend via IPC.
use crate::detection;

/// Detect the language of document content, optionally guided by a filename.
///
/// Returns a language ID string (e.g. "python", "json", "rust") or `null`
/// when detection is uncertain.
#[tauri::command]
pub fn detect_language(content: String, filename: Option<String>) -> Option<&'static str> {
    detection::detect_language(&content, filename.as_deref())
}

/// Detect language from a filename or path using extension/filename only.
///
/// Runs only Phase 1 of the detection pipeline (extension + FILENAME_MAP +
/// nginx regex), with no content scan. Returns a language ID string or `null`
/// when no mapping exists.
///
/// Use this instead of `detect_language` when no document content is available
/// (e.g. sidebar file cards, editor language pin on file open).
#[tauri::command]
pub fn detect_by_filename(filename: String) -> Option<&'static str> {
    detection::extension::detect_by_filename(&filename)
}
