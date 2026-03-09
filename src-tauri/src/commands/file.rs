use std::path::{Path, PathBuf};

use tauri::{Manager, path::BaseDirectory};

use crate::storage::{
    AppStorage, FileEventType, FileSource, RecentFileRecord, SETTING_NOTES_ROOT, normalize_path_key,
};

/// Maximum file size allowed to be opened: 200 MB.
const MAX_FILE_SIZE: u64 = 200 * 1024 * 1024;
const MANAGED_NOTES_DIRECTORY: &str = "Grayslate";
const MAX_RECENT_FILES_LIMIT: usize = 200;

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

fn resolve_notes_root_path(
    app: &tauri::AppHandle,
    storage: &AppStorage,
) -> Result<PathBuf, String> {
    if let Some(configured_path) = storage.get_setting(SETTING_NOTES_ROOT)? {
        let configured_path = PathBuf::from(configured_path);
        if !configured_path.is_absolute() {
            return Err("Configured notes root must be an absolute path.".to_string());
        }

        return Ok(configured_path);
    }

    resolve_default_notes_root_path(app)
}

fn classify_file_source(
    app: &tauri::AppHandle,
    storage: &AppStorage,
    path: &Path,
) -> Result<FileSource, String> {
    let notes_root = resolve_notes_root_path(app, storage)?;
    let notes_root_key = normalize_path_key(&notes_root)?;
    let path_key = normalize_path_key(path)?;
    let is_internal = path_key == notes_root_key || path_key.starts_with(&(notes_root_key + "/"));

    Ok(if is_internal {
        FileSource::Internal
    } else {
        FileSource::External
    })
}

fn clamp_recent_files_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(50).clamp(1, MAX_RECENT_FILES_LIMIT)
}

/// Read a file from disk and return its text content.
///
/// Returns an error string (forwarded to the frontend) when:
/// - the path cannot be stat-ed or read, or
/// - the file exceeds the 200 MB limit.
#[tauri::command]
pub async fn read_file_content(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    path: String,
) -> Result<String, String> {
    let metadata = std::fs::metadata(&path).map_err(|e| format!("Cannot access file: {}", e))?;

    if metadata.len() > MAX_FILE_SIZE {
        let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
        return Err(format!(
            "File is too large ({:.1} MB). The maximum allowed size is 200 MB.",
            size_mb
        ));
    }

    let content = std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;
    let source = classify_file_source(&app, storage.inner(), Path::new(&path))?;
    storage.record_file_event(Path::new(&path), source, FileEventType::Open)?;

    Ok(content)
}

#[tauri::command]
pub fn resolve_default_notes_root(app: tauri::AppHandle) -> Result<String, String> {
    let path = resolve_default_notes_root_path(&app)?;
    path.into_os_string()
        .into_string()
        .map_err(|_| "Default notes root contains invalid UTF-8.".to_string())
}

#[tauri::command]
pub fn resolve_notes_root(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
) -> Result<String, String> {
    let path = resolve_notes_root_path(&app, storage.inner())?;
    path.into_os_string()
        .into_string()
        .map_err(|_| "Notes root contains invalid UTF-8.".to_string())
}

#[tauri::command]
pub fn get_app_setting(
    storage: tauri::State<'_, AppStorage>,
    key: String,
) -> Result<Option<String>, String> {
    storage.get_setting(&key)
}

#[tauri::command]
pub fn set_app_setting(
    storage: tauri::State<'_, AppStorage>,
    key: String,
    value: Option<String>,
) -> Result<(), String> {
    if key == SETTING_NOTES_ROOT {
        if let Some(ref configured_path) = value {
            let configured_path = PathBuf::from(configured_path);
            if !configured_path.is_absolute() {
                return Err("Configured notes root must be an absolute path.".to_string());
            }
        }
    }

    storage.set_setting(&key, value.as_deref())
}

#[tauri::command]
pub fn get_recent_files(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    limit: Option<usize>,
) -> Result<Vec<RecentFileRecord>, String> {
    let limit = clamp_recent_files_limit(limit);
    let recent_files = storage.list_recent_files(limit)?;

    for recent_file in &recent_files {
        let path = PathBuf::from(&recent_file.path);
        let source = classify_file_source(&app, storage.inner(), &path)?;
        storage.refresh_tracked_file(&path, source)?;
    }

    storage.list_recent_files(limit)
}

#[tauri::command]
pub async fn write_file_content(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    path: String,
    content: String,
) -> Result<(), String> {
    let target_path = PathBuf::from(&path);
    validate_write_path(&target_path)?;

    let parent = target_path
        .parent()
        .ok_or_else(|| "Save path must have a parent directory.".to_string())?
        .to_path_buf();

    let path_for_write = target_path.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        std::fs::create_dir_all(&parent)
            .map_err(|error| format!("Failed to create parent directory: {}", error))?;
        std::fs::write(&path_for_write, content)
            .map_err(|error| format!("Failed to save file: {}", error))
    })
    .await
    .map_err(|error| format!("Failed to join file write task: {}", error))??;

    let source = classify_file_source(&app, storage.inner(), &target_path)?;
    storage.record_file_event(&target_path, source, FileEventType::Save)
}
