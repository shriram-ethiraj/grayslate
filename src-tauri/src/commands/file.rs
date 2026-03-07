use std::path::{Path, PathBuf};

use tauri::{Manager, path::BaseDirectory};

/// Maximum file size allowed to be opened: 200 MB.
const MAX_FILE_SIZE: u64 = 200 * 1024 * 1024;
const MANAGED_NOTES_DIRECTORY: &str = "Grayslate";

fn validate_write_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("Save path must be absolute.".to_string());
    }

    let Some(parent) = path.parent() else {
        return Err("Save path must have a parent directory.".to_string());
    };

    if parent.as_os_str().is_empty() {
        return Err("Save path must have a valid parent directory.".to_string());
    }

    Ok(())
}

fn resolve_default_notes_root_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let documents_dir = app
        .path()
        .resolve("", BaseDirectory::Document)
        .map_err(|error| format!("Unable to locate the Documents directory: {}", error))?;

    Ok(documents_dir.join(MANAGED_NOTES_DIRECTORY))
}

/// Read a file from disk and return its text content.
///
/// Returns an error string (forwarded to the frontend) when:
/// - the path cannot be stat-ed or read, or
/// - the file exceeds the 200 MB limit.
#[tauri::command]
pub async fn read_file_content(path: String) -> Result<String, String> {
    let metadata = std::fs::metadata(&path).map_err(|e| format!("Cannot access file: {}", e))?;

    if metadata.len() > MAX_FILE_SIZE {
        let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
        return Err(format!(
            "File is too large ({:.1} MB). The maximum allowed size is 200 MB.",
            size_mb
        ));
    }

    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))
}

#[tauri::command]
pub fn resolve_default_notes_root(app: tauri::AppHandle) -> Result<String, String> {
    let path = resolve_default_notes_root_path(&app)?;
    path.into_os_string()
        .into_string()
        .map_err(|_| "Default notes root contains invalid UTF-8.".to_string())
}

#[tauri::command]
pub async fn write_file_content(path: String, content: String) -> Result<(), String> {
    let target_path = PathBuf::from(&path);
    validate_write_path(&target_path)?;

    let parent = target_path
        .parent()
        .ok_or_else(|| "Save path must have a parent directory.".to_string())?
        .to_path_buf();

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        std::fs::create_dir_all(&parent)
            .map_err(|error| format!("Failed to create parent directory: {}", error))?;
        std::fs::write(&target_path, content)
            .map_err(|error| format!("Failed to save file: {}", error))
    })
    .await
    .map_err(|error| format!("Failed to join file write task: {}", error))?
}
