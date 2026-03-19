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
 *   5. Records the file event in storage (same as write_file_content).
 *   6. Returns the final absolute path to the frontend.
 */
use crate::{
    filesystem::{classify_file_source, resolve_notes_root_path, sanitize_filename, unique_path_in_dir},
    naming::{fallback_stem, language_to_extension, suggest_stem},
    storage::{AppStorage, FileEventType},
};

// ---------------------------------------------------------------------------
// save_untitled_slate
// ---------------------------------------------------------------------------

/// Saves an untitled document to the notes root with a smart filename derived
/// from its content.
///
/// Returns the final absolute path that was written.
#[tauri::command]
pub async fn save_untitled_slate(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    content: String,
    language_hint: String,
) -> Result<String, String> {
    // Resolve the configured notes root (same logic as the existing save flow).
    let notes_root = resolve_notes_root_path(&app, storage.inner())?;

    // Derive a smart stem from the content.
    let stem = suggest_stem(&content, &language_hint).unwrap_or_else(fallback_stem);

    let extension = language_to_extension(&language_hint);

    // Build a collision-free path inside notes_root.
    let base_name = if extension.is_empty() {
        stem
    } else {
        format!("{}.{}", stem, extension)
    };
    let base_name = sanitize_filename(&base_name);
    let target_path = unique_path_in_dir(&notes_root, &base_name);

    // Write the file (create parent dirs just in case).
    let path_for_write = target_path.clone();
    let content_for_write = content.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        if let Some(parent) = path_for_write.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create notes root: {}", e))?;
        }
        std::fs::write(&path_for_write, content_for_write)
            .map_err(|e| format!("Failed to save file: {}", e))
    })
    .await
    .map_err(|e| format!("Failed to join file write task: {}", e))??;

    // Record the save event in storage (mirrors write_file_content behaviour).
    let source = classify_file_source(&app, storage.inner(), &target_path)?;
    storage.record_file_event(&target_path, source, FileEventType::Save)?;

    target_path
        .into_os_string()
        .into_string()
        .map_err(|_| "Saved path contains invalid UTF-8.".to_string())
}

// ---------------------------------------------------------------------------
// suggest_slate_name  (lightweight helper for Save As default path)
// ---------------------------------------------------------------------------

/// Returns a suggested full filename (stem + extension) without writing
/// anything to disk.  Used by the frontend to pre-populate the Save As picker.
#[tauri::command]
pub fn suggest_slate_name(content: String, language_hint: String) -> String {
    let stem = suggest_stem(&content, &language_hint).unwrap_or_else(fallback_stem);
    let extension = language_to_extension(&language_hint);
    if extension.is_empty() {
        stem
    } else {
        format!("{}.{}", stem, extension)
    }
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
        let name = suggest_slate_name(
            r#"{"userId":1,"name":"Alice"}"#.to_string(),
            "json".to_string(),
        );
        assert!(name.ends_with(".json"), "got: {name}");
    }
}
