use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::filesystem::{classify_file_source, resolve_default_notes_root_path, resolve_notes_root_path};
use crate::storage::{
    AppStorage, FileEventType, RecentFileRecord, SETTING_NOTES_ROOT,
};

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
        let mut active_reads = self.active_reads.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
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
        let mut active_reads = self.active_reads.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        if let Some(previous) = active_reads.remove(window_label) {
            previous.cancelled.store(true, Ordering::Relaxed);
        }
    }

    fn finish_request(&self, window_label: &str, request_id: u64) {
        let mut active_reads = self.active_reads.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

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
    std::str::from_utf8(&bytes)
        .map_err(|error| format!("Failed to read file: {}", error))?;

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
pub fn prepare_file_open(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    path: String,
) -> Result<RecentFileRecord, String> {
    let path_buf = PathBuf::from(&path);
    let source = classify_file_source(&app, storage.inner(), &path_buf)?;
    storage.record_file_event(&path_buf, source, FileEventType::Open)?;
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
    storage.record_file_event(&target_path, source, FileEventType::Save)
}
