/**
 * naming.rs (commands)
 *
 * Tauri command that handles the first save of an untitled document with
 * smart content-based filename suggestion.
 *
 * The frontend calls `save_untitled_slate` instead of building a path itself.
 * This command:
 *   1. Suggests a filename stem from the content via naming::suggest_stem.
 *   2. Picks the right extension via naming::language_to_extension.
 *   3. Resolves a collision-free path inside notes_root.
 *   4. Creates parent directories and writes the file.
 *   5. Records the file update in storage (same as write_file_content).
 *   6. Returns the final absolute path to the frontend.
 */
use std::path::PathBuf;

use crate::{
    autosave::atomic_create_to_disk,
    document::{canonical_notes_root, DocumentRegistry, DocumentRights},
    filesystem::{sanitize_filename, unique_path_in_dir},
    naming::{fallback_stem, language_to_extension, suggest_stem_auto},
    storage::{AppStorage, FileSource},
};

use tauri::Emitter;

use super::RECENT_FILES_UPDATED_EVENT;

/// Result of saving an untitled slate — includes both the path and the
/// detected language so the frontend can update its state in one IPC call.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub path: String,
    pub document_id: String,
    pub document_generation: u64,
    pub source: String,
    pub detected_language: String,
}

/// Result of suggesting a name — includes both the filename and the
/// detected language.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestResult {
    pub filename: String,
    pub detected_language: String,
}

// ---------------------------------------------------------------------------
// save_untitled_slate
// ---------------------------------------------------------------------------

/// Saves an untitled document to the notes root with a smart filename derived
/// from its content.
///
/// When `language_hint` is `"auto"` or empty, the backend auto-detects the
/// language from the content first.
///
/// Returns both the final path and the effective language.
#[tauri::command]
pub async fn save_untitled_slate(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    autosave: tauri::State<'_, crate::autosave::AutosaveRegistry>,
    window: tauri::Window,
    content: String,
    language_hint: String,
) -> Result<SaveResult, String> {
    let result = save_new_slate_to_disk(
        &app,
        storage.inner(),
        documents.inner(),
        window.label(),
        &content,
        &language_hint,
    )
    .await?;
    autosave.register_authorized(
        window.label(),
        PathBuf::from(&result.path),
        FileSource::Slates,
        language_hint,
        result.document_id.clone(),
        result.document_generation,
    );
    Ok(result)
}

/// Core logic for saving untitled content as a new slate file.
/// Shared between `save_untitled_slate` (explicit Save) and autosave's
/// first-save-of-untitled flow.
pub async fn save_new_slate_to_disk(
    app: &tauri::AppHandle,
    storage: &AppStorage,
    documents: &DocumentRegistry,
    window_label: &str,
    content: &str,
    language_hint: &str,
) -> Result<SaveResult, String> {
    let notes_root = canonical_notes_root(app, storage, true)?;

    let (stem, effective_language) = suggest_stem_auto(content, language_hint, None);
    let stem = stem.unwrap_or_else(fallback_stem);

    let extension = language_to_extension(&effective_language);

    let base_name = if extension.is_empty() {
        stem
    } else {
        format!("{}.{}", stem, extension)
    };
    let base_name = sanitize_filename(&base_name);
    let target_path = unique_path_in_dir(&notes_root, &base_name);
    let granted = documents.grant_new(
        window_label,
        &target_path,
        FileSource::Slates,
        DocumentRights::tracked(FileSource::Slates),
    )?;

    let path_for_write = target_path.clone();
    let content_for_write = content.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        atomic_create_to_disk(&path_for_write, &content_for_write)
    })
    .await
    .map_err(|e| format!("Failed to join file write task: {}", e))??;

    let created = documents.mark_created(window_label, &granted.id, granted.generation)?;
    storage.record_file_update(&target_path, FileSource::Slates)?;
    let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());

    target_path
        .into_os_string()
        .into_string()
        .map(|path| SaveResult {
            path,
            document_id: created.id,
            document_generation: created.generation,
            source: FileSource::Slates.as_str().to_string(),
            detected_language: effective_language,
        })
        .map_err(|_| "Saved path contains invalid UTF-8.".to_string())
}

// ---------------------------------------------------------------------------
// Shared naming helper
// ---------------------------------------------------------------------------

/// Derives a suggested full filename (`stem.extension`) from pre-loaded content
/// and a language hint.  Auto-detects language when hint is "auto" or empty.
/// Returns both the filename and the effective language.
fn build_suggested_name(content: &str, language_hint: &str) -> (String, String) {
    let (stem, effective_language) = suggest_stem_auto(content, language_hint, None);
    let stem = stem.unwrap_or_else(fallback_stem);
    let extension = language_to_extension(&effective_language);
    let filename = if extension.is_empty() {
        stem
    } else {
        format!("{}.{}", stem, extension)
    };
    (filename, effective_language)
}

// ---------------------------------------------------------------------------
// suggest_slate_name  (Save As — frontend already has the content)
// ---------------------------------------------------------------------------

/// Returns a suggested full filename without writing anything to disk.
/// Used by the frontend to pre-populate the Save As picker.
/// Also returns the detected language when hint is "auto".
#[tauri::command]
pub fn suggest_slate_name(content: String, language_hint: String) -> SuggestResult {
    let (filename, detected_language) = build_suggested_name(&content, &language_hint);
    SuggestResult {
        filename,
        detected_language,
    }
}

// ---------------------------------------------------------------------------
// suggest_name_for_file  (Rename dialog — backend reads file from disk)
// ---------------------------------------------------------------------------

/// Reads a bounded sample of an existing file, detects its language from the
/// file content, runs the naming pipeline, and returns a suggested filename
/// (stem + extension).
///
/// The frontend passes only the current document grant; Rust resolves the path
/// and performs the bounded I/O so no large content crosses the IPC boundary.
///
/// Language detection uses the same "auto" content-driven cascade as the
/// untitled-save flow — the file extension is intentionally NOT used as a hint
/// so that mis-named files (e.g. notes.txt containing Perl) still get the
/// correct extension in the suggestion.
#[tauri::command]
pub fn suggest_name_for_file(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    document_id: String,
    document_generation: u64,
) -> Result<String, String> {
    use std::io::Read;

    let document = documents.resolve(
        window.label(),
        &document_id,
        document_generation,
        crate::document::DocumentAccess::Read,
    )?;
    crate::document::revalidate_source_authority(&app, storage.inner(), &document)?;
    let file_path = &document.path;

    // Read only what the naming pipeline can use to keep this fast.
    // Gracefully fall back to an empty string for binary or unreadable files.
    const READ_LIMIT: u64 = 8_192;
    let mut raw_bytes = Vec::with_capacity(READ_LIMIT as usize);
    let _ = crate::document::open_authorized_read(file_path).and_then(|mut f| {
        f.by_ref()
            .take(READ_LIMIT)
            .read_to_end(&mut raw_bytes)
            .map_err(|error| error.to_string())
    });
    let content = String::from_utf8_lossy(&raw_bytes);

    // "auto" triggers the family-first detection pipeline:
    // extension → shebang → structural → family → scoring → disambiguation.
    Ok(build_suggested_name(&content, "auto").0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("grayslate_naming_test_{ns}_{n}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolve_available_path_first_slot() {
        let dir = tmp_dir();
        let path = unique_path_in_dir(&dir, "my-file.json");
        assert_eq!(path.file_name().unwrap(), "my-file.json");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resolve_available_path_collision() {
        let dir = tmp_dir();
        // Occupy the first slot.
        fs::write(dir.join("my-file.json"), "{}").unwrap();
        let path = unique_path_in_dir(&dir, "my-file.json");
        assert_eq!(path.file_name().unwrap(), "my-file-2.json");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resolve_available_path_multiple_collisions() {
        let dir = tmp_dir();
        fs::write(dir.join("report.sql"), "").unwrap();
        fs::write(dir.join("report-2.sql"), "").unwrap();
        fs::write(dir.join("report-3.sql"), "").unwrap();
        let path = unique_path_in_dir(&dir, "report.sql");
        assert_eq!(path.file_name().unwrap(), "report-4.sql");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn suggest_slate_name_returns_filename() {
        let result = suggest_slate_name(
            r#"{"userId":1,"name":"Alice"}"#.to_string(),
            "json".to_string(),
        );
        assert!(
            result.filename.ends_with(".json"),
            "got: {}",
            result.filename
        );
    }
}
