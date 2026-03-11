use std::path::{Path, PathBuf};

use tauri::path::BaseDirectory;
use tauri::Manager;

use crate::storage::{AppStorage, FileSource, SETTING_NOTES_ROOT, normalize_path_key};

const MANAGED_NOTES_DIRECTORY: &str = "Grayslate";

pub fn resolve_default_notes_root_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let documents_dir = app
        .path()
        .resolve("", BaseDirectory::Document)
        .map_err(|error| format!("Unable to locate the Documents directory: {}", error))?;

    Ok(documents_dir.join(MANAGED_NOTES_DIRECTORY))
}

pub fn resolve_notes_root_path(
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

pub fn classify_file_source(
    app: &tauri::AppHandle,
    storage: &AppStorage,
    path: &Path,
) -> Result<FileSource, String> {
    let notes_root = resolve_notes_root_path(app, storage)?;
    let notes_root_key = normalize_path_key(&notes_root)?;
    let path_key = normalize_path_key(path)?;
    let is_slates = path_key == notes_root_key || path_key.starts_with(&(notes_root_key + "/"));

    Ok(if is_slates {
        FileSource::Slates
    } else {
        FileSource::Local
    })
}