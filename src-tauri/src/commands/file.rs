use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use tauri::Emitter;

use crate::filesystem::{
    classify_file_source, resolve_default_notes_root_path, resolve_notes_root_path,
    sanitize_filename, unique_path_in_dir,
};
use crate::storage::{
    normalize_path_key, AppStorage, FileSource, RecentFileRecord, SETTING_FONT_SIZE,
    SETTING_NOTES_ROOT, SETTING_SIDEBAR_OPEN, SETTING_SIDEBAR_WIDTH, SETTING_THEME,
    SETTING_WORD_WRAP,
};

use super::RECENT_FILES_UPDATED_EVENT;

/// Maximum file size allowed to be opened: 200 MB.
const MAX_FILE_SIZE: u64 = 200 * 1024 * 1024;
const FILE_READ_CHUNK_SIZE: usize = 256 * 1024;
const FILE_READ_CANCELLED_MESSAGE: &str = "File read cancelled.";
const MAX_RECENT_FILES_LIMIT: usize = 200;

#[derive(Clone)]
struct ActiveFileRead {
    request_id: u64,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct FileReadCancellationRegistry {
    active_reads: Mutex<HashMap<String, ActiveFileRead>>,
}

impl FileReadCancellationRegistry {
    fn begin_request(&self, window_label: &str, request_id: u64) -> Arc<AtomicBool> {
        let mut active_reads = self
            .active_reads
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let cancelled = Arc::new(AtomicBool::new(false));

        if let Some(previous) = active_reads.insert(
            window_label.to_string(),
            ActiveFileRead {
                request_id,
                cancelled: Arc::clone(&cancelled),
            },
        ) {
            previous.cancelled.store(true, Ordering::Relaxed);
        }

        cancelled
    }

    fn cancel_window_request(&self, window_label: &str) {
        let mut active_reads = self
            .active_reads
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if let Some(previous) = active_reads.remove(window_label) {
            previous.cancelled.store(true, Ordering::Relaxed);
        }
    }

    fn finish_request(&self, window_label: &str, request_id: u64) {
        let mut active_reads = self
            .active_reads
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let should_remove = active_reads
            .get(window_label)
            .map(|active| active.request_id == request_id)
            .unwrap_or(false);

        if should_remove {
            active_reads.remove(window_label);
        }
    }
}

fn ensure_read_not_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(FILE_READ_CANCELLED_MESSAGE.to_string());
    }

    Ok(())
}

fn read_file_bytes_cancellable(path: &Path, cancelled: &AtomicBool) -> Result<Vec<u8>, String> {
    ensure_read_not_cancelled(cancelled)?;

    let file = File::open(path).map_err(|error| format!("Failed to read file: {}", error))?;
    let mut reader = BufReader::new(file);
    let mut bytes = Vec::new();
    let mut chunk = vec![0_u8; FILE_READ_CHUNK_SIZE];

    loop {
        ensure_read_not_cancelled(cancelled)?;

        let bytes_read = reader
            .read(&mut chunk)
            .map_err(|error| format!("Failed to read file: {}", error))?;

        if bytes_read == 0 {
            break;
        }

        bytes.extend_from_slice(&chunk[..bytes_read]);
    }

    ensure_read_not_cancelled(cancelled)?;

    // Validate UTF-8 without converting to String — avoids an allocation.
    std::str::from_utf8(&bytes).map_err(|error| format!("Failed to read file: {}", error))?;

    Ok(bytes)
}

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

fn clamp_recent_files_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(50).clamp(1, MAX_RECENT_FILES_LIMIT)
}

/// Read a file from disk and return its content as raw bytes.
///
/// Returns a `tauri::ipc::Response` with the raw UTF-8 bytes, bypassing JSON
/// serialization. The frontend receives an `ArrayBuffer` and decodes it with
/// `TextDecoder`, avoiding the overhead of JSON-escaping up to 200 MB of text.
///
/// Returns an error string (forwarded to the frontend) when:
/// - the path cannot be stat-ed or read,
/// - the file exceeds the 200 MB limit, or
/// - the file is not valid UTF-8.
#[tauri::command]
pub async fn read_file_content(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    cancellations: tauri::State<'_, FileReadCancellationRegistry>,
    window: tauri::Window,
    path: String,
    request_id: u64,
) -> Result<tauri::ipc::Response, String> {
    let window_label = window.label().to_string();
    let cancellation_flag = cancellations.begin_request(&window_label, request_id);
    let path_buf = PathBuf::from(&path);

    let result = async {
        let metadata = std::fs::metadata(&path_buf)
            .map_err(|error| format!("Cannot access file: {}", error))?;

        ensure_read_not_cancelled(cancellation_flag.as_ref())?;

        if metadata.len() > MAX_FILE_SIZE {
            let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
            return Err(format!(
                "File is too large ({:.1} MB). The maximum allowed size is 200 MB.",
                size_mb
            ));
        }

        let read_path = path_buf.clone();
        let read_cancelled = Arc::clone(&cancellation_flag);
        let bytes = tauri::async_runtime::spawn_blocking(move || {
            read_file_bytes_cancellable(&read_path, read_cancelled.as_ref())
        })
        .await
        .map_err(|error| format!("Failed to join file read task: {}", error))??;

        ensure_read_not_cancelled(cancellation_flag.as_ref())?;

       if let Ok(source) = classify_file_source(&app, storage.inner(), &path_buf) {
            let _ = storage.record_file_event(&path_buf, source);
        }
        let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());

        Ok(tauri::ipc::Response::new(bytes))
    }
    .await;

    cancellations.finish_request(&window_label, request_id);

    result
}

#[tauri::command]
pub fn cancel_file_read(
    cancellations: tauri::State<'_, FileReadCancellationRegistry>,
    window: tauri::Window,
) {
    cancellations.cancel_window_request(window.label());
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
pub fn get_all_settings(
    storage: tauri::State<'_, AppStorage>,
) -> Result<std::collections::HashMap<String, String>, String> {
    storage.get_all_settings()
}

#[tauri::command]
pub fn set_app_setting(
    storage: tauri::State<'_, AppStorage>,
    key: String,
    value: Option<String>,
) -> Result<(), String> {
    match key.as_str() {
        SETTING_NOTES_ROOT => {
            if let Some(ref configured_path) = value {
                let configured_path = PathBuf::from(configured_path);
                if !configured_path.is_absolute() {
                    return Err("Configured notes root must be an absolute path.".to_string());
                }
            }
        }
        SETTING_THEME => {
            if let Some(ref theme) = value {
                if theme != "dark" && theme != "light" {
                    return Err("Theme must be \"dark\" or \"light\".".to_string());
                }
            }
        }
        SETTING_FONT_SIZE => {
            if let Some(ref size) = value {
                let parsed: i32 = size.parse().map_err(|_| {
                    format!("Font size must be a number, got \"{}\".", size)
                })?;
                if !(10..=24).contains(&parsed) {
                    return Err(format!("Font size must be between 10 and 24, got {}.", parsed));
                }
            }
        }
        SETTING_WORD_WRAP => {
            if let Some(ref wrap) = value {
                if wrap != "true" && wrap != "false" {
                    return Err("Word wrap must be \"true\" or \"false\".".to_string());
                }
            }
        }
        SETTING_SIDEBAR_WIDTH => {
            if let Some(ref width) = value {
                let parsed: i32 = width.parse().map_err(|_| {
                    format!("Sidebar width must be a number, got \"{}\".", width)
                })?;
                if !(15..=30).contains(&parsed) {
                    return Err(format!(
                        "Sidebar width must be between 15 and 30, got {}.",
                        parsed
                    ));
                }
            }
        }
        SETTING_SIDEBAR_OPEN => {
            if let Some(ref open) = value {
                if open != "true" && open != "false" {
                    return Err("Sidebar open must be \"true\" or \"false\".".to_string());
                }
            }
        }
        _ => {}
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

    // Keep the tracking table in sync with the filesystem before returning
    // results so newly-added / externally-deleted slates files are reflected
    // in the sidebar immediately.
    sync_notes_tracking(&app, storage.inner());

    let recent_files = storage.list_recent_files(limit)?;

    for recent_file in &recent_files {
        let path = PathBuf::from(&recent_file.path);
        let source = classify_file_source(&app, storage.inner(), &path)?;
        storage.refresh_tracked_file(&path, source)?;
    }

    // Prune entries that no longer exist on disk so the sidebar stays clean.
    let refreshed = storage.list_recent_files(limit)?;
    let mut result = Vec::new();
    for file in refreshed {
        let path = PathBuf::from(&file.path);
        if path.exists() {
            result.push(file);
        } else {
            let _ = storage.delete_tracked_file(&path);
        }
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Notes-directory sync helpers
// ---------------------------------------------------------------------------

/// Lists only top-level regular files in `root`.  Skips hidden files (names
/// starting with `.`) and silently ignores entry-level errors so a single
/// unreadable or broken item never aborts the scan.
fn list_top_level_files(root: &Path) -> Vec<PathBuf> {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        // Skip hidden files (.git, .DS_Store, .env, etc.)
        if name.starts_with('.') {
            continue;
        }

        if entry.file_type().map_or(false, |ft| ft.is_file()) {
            files.push(path);
        }
    }
    files
}

/// Diff the notes directory against the `tracked_files` table and reconcile
/// the two: add newly-discovered files, remove entries for files that were
/// deleted from disk, and refresh metadata for files that still exist.
///
/// Only touches rows whose `source` is `'slates'` — local (external) files
/// are left unchanged.
///
/// This function is intentionally infallible: individual errors are logged
/// (via `eprintln!`) and ignored so the sync never causes the sidebar to
/// show an error.
fn sync_notes_tracking(app: &tauri::AppHandle, storage: &AppStorage) {
    let notes_root = match resolve_notes_root_path(app, storage) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("sync_notes_tracking: failed to resolve notes root: {}", e);
            return;
        }
    };

    // Fresh install — the notes directory hasn't been created yet.
    if !notes_root.exists() {
        return;
    }

    let disk_files = list_top_level_files(&notes_root);

    let mut tracked_map = match storage.list_slates_path_map() {
        Ok(map) => map,
        Err(e) => {
            eprintln!("sync_notes_tracking: failed to read tracked files: {}", e);
            return;
        }
    };

    for disk_file in &disk_files {
        let path_key = match normalize_path_key(disk_file) {
            Ok(key) => key,
            Err(e) => {
                eprintln!(
                    "sync_notes_tracking: failed to normalize path '{}': {}",
                    disk_file.display(),
                    e
                );
                continue;
            }
        };
        if !tracked_map.contains_key(&path_key) {
            if let Err(e) = storage.upsert_slates_file_for_sync(disk_file) {
                eprintln!(
                    "sync_notes_tracking: failed to upsert '{}': {}",
                    disk_file.display(),
                    e
                );
            }
        }
        tracked_map.remove(&path_key);
    }

    // Entries still in the map exist in the DB but not on disk — remove them.
    for (_key, path) in &tracked_map {
        if let Err(e) = storage.delete_tracked_file(Path::new(path)) {
            eprintln!(
                "sync_notes_tracking: failed to delete tracked file '{}': {}",
                path,
                e
            );
        }
    }
}

#[tauri::command]
pub fn prepare_file_open(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    path: String,
) -> Result<RecentFileRecord, String> {
    let path_buf = PathBuf::from(&path);
    let source = classify_file_source(&app, storage.inner(), &path_buf)?;
    storage.record_file_event(&path_buf, source)?;
    storage
        .get_tracked_file(&path_buf)?
        .ok_or_else(|| "Failed to resolve prepared file entry.".to_string())
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
    storage.record_file_event(&target_path, source)?;
    let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());
    Ok(())
}

// ---------------------------------------------------------------------------
// File management helpers (rename / delete / duplicate)
// ---------------------------------------------------------------------------

/// Validates a proposed filename: must be non-empty, contain no path
/// separators, and have no ASCII control characters.
fn validate_new_filename(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("File name cannot be empty.".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err("File name cannot contain path separators.".to_string());
    }
    if trimmed.chars().any(|c| (c as u32) < 32 || c == '\x7f') {
        return Err("File name contains invalid characters.".to_string());
    }
    Ok(())
}

/// Strips any trailing `-copy` or `-copy-<N>` suffix from a stem and returns
/// the root part.  For example:
///   `"file-copy"`   → `"file"`
///   `"file-copy-3"` → `"file"`
///   `"file"`        → `"file"` (unchanged)
fn strip_copy_suffix(stem: &str) -> &str {
    // "-copy-<digits>" (numbered copy)
    if let Some((before, suffix)) = stem.rsplit_once("-copy-") {
        if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
            return before;
        }
    }
    // bare "-copy"
    if let Some(before) = stem.strip_suffix("-copy") {
        return before;
    }
    stem
}

/// Generates a base name for a duplicate copy.
///
/// Always strips any existing `-copy` / `-copy-N` suffix back to the root
/// stem, then appends `-copy`.  The caller is responsible for finding the
/// next available slot with `next_copy_path_in_dir`.
///
/// - `"file.md"`        → `"file-copy.md"`
/// - `"file-copy.md"`   → `"file-copy.md"`
/// - `"file-copy-3.md"` → `"file-copy.md"`
fn make_copy_name(src_name: &str) -> String {
    let (stem, ext) = if let Some(pos) = src_name.rfind('.') {
        (&src_name[..pos], &src_name[pos..])
    } else {
        (src_name, "")
    };
    let base = strip_copy_suffix(stem);
    format!("{}-copy{}", base, ext)
}

/// Finds the next available copy path in `dir`.
///
/// Slot 0 (no number): `<base>-copy.<ext>`
/// Slot 1:             `<base>-copy-1.<ext>`
/// Slot 2:             `<base>-copy-2.<ext>`
/// …
///
/// `copy_name` must already be of the form `"<base>-copy.<ext>"` as produced
/// by `make_copy_name`.
fn next_copy_path_in_dir(dir: &Path, copy_name: &str) -> PathBuf {
    let candidate = dir.join(copy_name);
    if !candidate.exists() {
        return candidate;
    }

    let (stem, ext) = if let Some(pos) = copy_name.rfind('.') {
        (&copy_name[..pos], &copy_name[pos..])
    } else {
        (copy_name, "")
    };

    // Numbered slots start at 1: file-copy-1, file-copy-2, …
    let mut counter = 1u32;
    loop {
        let name = format!("{}-{}{}", stem, counter, ext);
        let path = dir.join(&name);
        if !path.exists() {
            return path;
        }
        counter += 1;
    }
}

/// Returns `Ok(())` when `path` is absolute and belongs to the Grayslate
/// notes root (source == Slates).  Returns a user-visible error otherwise.
/// Rename and delete are restricted to slate files managed by the app.
fn require_slate_file(
    app: &tauri::AppHandle,
    storage: &AppStorage,
    path: &Path,
) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("File path must be absolute.".to_string());
    }
    let source = classify_file_source(app, storage, path)?;
    if source != FileSource::Slates {
        return Err(
            "Rename and delete are only available for Grayslate slate files.".to_string(),
        );
    }
    Ok(())
}

/// Remove a local (external) file from sidebar tracking without deleting
/// it from disk.  Returns an error if the path is a managed slate file.
#[tauri::command]
pub fn untrack_local_file(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    path: String,
) -> Result<(), String> {
    let target = PathBuf::from(&path);
    if !target.is_absolute() {
        return Err("File path must be absolute.".to_string());
    }
    let source = classify_file_source(&app, storage.inner(), &target)?;
    if source == FileSource::Slates {
        return Err("Cannot untrack a Grayslate slate file. Use Delete instead.".to_string());
    }

    storage.delete_tracked_file(&target)?;
    let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());
    Ok(())
}

/// Permanently delete a slate file from disk and remove it from tracking.
/// Returns an error if `path` is not a managed slate file.
#[tauri::command]
pub async fn delete_file(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    path: String,
) -> Result<(), String> {
    let target = PathBuf::from(&path);
    require_slate_file(&app, storage.inner(), &target)?;

    let target_clone = target.clone();
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::remove_file(&target_clone)
            .map_err(|e| format!("Failed to delete file: {}", e))
    })
    .await
    .map_err(|e| format!("Delete task failed: {}", e))??;

    // Best-effort removal from tracking; ignore errors if the row was never
    // stored.
    let _ = storage.delete_tracked_file(&target);
    let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());

    Ok(())
}

/// Rename a slate file.  `new_name` is the bare filename (no path separators).
/// The name is sanitized and slugified before use: unsafe characters and
/// whitespace runs are collapsed into hyphens.  If a file with the resulting
/// name already exists in the same directory, a numeric suffix is automatically
/// appended (`name-2.ext`, `name-3.ext`, …) so the operation always
/// succeeds.  Returns the absolute path of the renamed file.
#[tauri::command]
pub async fn rename_file(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    path: String,
    new_name: String,
) -> Result<String, String> {
    let old_path = PathBuf::from(&path);
    require_slate_file(&app, storage.inner(), &old_path)?;

    let sanitized_name = sanitize_filename(&new_name);
    validate_new_filename(&sanitized_name)?;

    let parent = old_path
        .parent()
        .ok_or_else(|| "File has no parent directory.".to_string())?
        .to_path_buf();

    let new_path = unique_path_in_dir(&parent, &sanitized_name);
    let new_path_str = new_path.to_string_lossy().to_string();

    let old_clone = old_path.clone();
    let new_clone = new_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::rename(&old_clone, &new_clone)
            .map_err(|e| format!("Failed to rename file: {}", e))
    })
    .await
    .map_err(|e| format!("Rename task failed: {}", e))??;

    storage.rename_tracked_file(&old_path, &new_path)?;
    let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());

    Ok(new_path_str)
}

/// Duplicate a local file into the Grayslate slates directory.
///
/// The file content is copied verbatim; the destination filename keeps the
/// same stem-and-extension but is placed in the managed notes root and given
/// a `-copy` suffix.  The new file is recorded in storage as a slate so it
/// appears in the sidebar immediately.  Returns the absolute path of the new
/// copy.
#[tauri::command]
pub async fn duplicate_local_file_as_slate(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    path: String,
) -> Result<String, String> {
    let src = PathBuf::from(&path);
    if !src.is_absolute() {
        return Err("File path must be absolute.".to_string());
    }
    if !src.exists() {
        return Err("Source file does not exist.".to_string());
    }

    let src_name = src
        .file_name()
        .ok_or_else(|| "File has no name.".to_string())?
        .to_string_lossy()
        .to_string();

    let notes_root =
        crate::filesystem::resolve_notes_root_path(&app, storage.inner())?;
    if !notes_root.exists() {
        std::fs::create_dir_all(&notes_root)
            .map_err(|e| format!("Failed to create slates directory: {}", e))?;
    }

    let copy_name = make_copy_name(&src_name);
    let dest = next_copy_path_in_dir(&notes_root, &copy_name);
    let dest_str = dest.to_string_lossy().to_string();

    let src_clone = src.clone();
    let dest_clone = dest.clone();
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::copy(&src_clone, &dest_clone)
            .map(|_| ())
            .map_err(|e| format!("Failed to copy file: {}", e))
    })
    .await
    .map_err(|e| format!("Duplicate task failed: {}", e))??;

    storage.record_file_event(&dest, crate::storage::FileSource::Slates)?;
    let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());

    Ok(dest_str)
}

/// Duplicate a file, placing the copy in the same directory with a `(copy)`
/// suffix in its name.  The duplicate is recorded in storage so it appears in
/// the sidebar immediately.  Works on any file (slates and local), as the copy
/// is always created alongside the original.  Returns the absolute path of the
/// new copy.
#[tauri::command]
pub async fn duplicate_file(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    path: String,
) -> Result<String, String> {
    let src = PathBuf::from(&path);
    if !src.is_absolute() {
        return Err("File path must be absolute.".to_string());
    }
    if !src.exists() {
        return Err("Source file does not exist.".to_string());
    }

    let parent = src
        .parent()
        .ok_or_else(|| "File has no parent directory.".to_string())?
        .to_path_buf();

    let src_name = src
        .file_name()
        .ok_or_else(|| "File has no name.".to_string())?
        .to_string_lossy()
        .to_string();

    let copy_name = make_copy_name(&src_name);
    let dest = next_copy_path_in_dir(&parent, &copy_name);
    let dest_str = dest.to_string_lossy().to_string();

    let src_clone = src.clone();
    let dest_clone = dest.clone();
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::copy(&src_clone, &dest_clone)
            .map(|_| ())
            .map_err(|e| format!("Failed to copy file: {}", e))
    })
    .await
    .map_err(|e| format!("Duplicate task failed: {}", e))??;

    let source = classify_file_source(&app, storage.inner(), &dest)?;
    storage.record_file_event(&dest, source)?;
    let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());

    Ok(dest_str)
}
