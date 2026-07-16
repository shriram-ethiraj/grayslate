/**
 * autosave.rs
 *
 * Backend-driven autosave engine for slate files.
 *
 * Architecture:
 *   - A background thread runs a periodic timer (every 500 ms).
 *   - The frontend sends lightweight `autosave_notify_changed(generation)`
 *     notifications (no content) whenever the editor document changes.
 *   - When the timer detects an idle pause (≥ 1.5 s) or a max-latency
 *     ceiling (≥ 10 s of unsaved changes), it triggers a save:
 *       • CSV table mode: serializes directly from CsvSession (no FE roundtrip).
 *       • Text mode: emits a `autosave://request-content` event to the FE,
 *         which responds by calling `autosave_submit_content` with the
 *         serialized document content.
 *   - Generation tracking prevents stale writes: if new edits arrive during
 *     a save roundtrip, another save is automatically scheduled.
 *   - File writes use atomic temp-file + rename for crash safety.
 */
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::commands::csv::CsvSessionRegistry;
use crate::commands::RECENT_FILES_UPDATED_EVENT;
use crate::document::{revalidate_source_authority, DocumentAccess, DocumentRegistry};
use crate::storage::{AppStorage, FileSource};

// ---------------------------------------------------------------------------
// Constants (tunable)
// ---------------------------------------------------------------------------

/// Save 1.5 s after the user stops typing.
const IDLE_DEBOUNCE_MS: u64 = 1_500;

/// Force a save every 10 s during continuous typing bursts.
const MAX_LATENCY_MS: u64 = 10_000;

/// How often the background thread checks for pending saves.
const TIMER_TICK_MS: u64 = 500;

/// If the FE doesn't respond to a content request within this window,
/// clear `save_in_flight` so the next tick retries.
const CONTENT_REQUEST_TIMEOUT_MS: u64 = 5_000;

// ---------------------------------------------------------------------------
// Event names
// ---------------------------------------------------------------------------

pub const AUTOSAVE_REQUEST_CONTENT_EVENT: &str = "autosave://request-content";
pub const AUTOSAVE_DOCUMENT_CREATED_EVENT: &str = "autosave://document-created";
pub const AUTOSAVE_FLUSH_BEFORE_CLOSE_EVENT: &str = "autosave://flush-before-close";

// ---------------------------------------------------------------------------
// Event payloads
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentRequestPayload {
    pub request_id: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DocumentCreatedPayload {
    pub path: String,
    pub document_id: String,
    pub document_generation: u64,
    pub detected_language: String,
}

// ---------------------------------------------------------------------------
// Per-document state
// ---------------------------------------------------------------------------

pub struct AutosaveDocument {
    pub path: Option<PathBuf>,
    pub document_id: Option<String>,
    pub document_generation: Option<u64>,
    pub source: FileSource,
    pub generation: u64,
    pub last_saved_generation: u64,
    pub last_notified_at: Option<Instant>,
    pub last_saved_at: Option<Instant>,
    pub save_in_flight: bool,
    pub csv_table_active: bool,
    pub language_hint: String,
    pub pending_request_id: Option<u64>,
    pending_request_at: Option<Instant>,
    next_request_id: u64,
}

impl AutosaveDocument {
    pub fn new(path: Option<PathBuf>, source: FileSource, language_hint: String) -> Self {
        AutosaveDocument {
            path,
            document_id: None,
            document_generation: None,
            source,
            generation: 0,
            last_saved_generation: 0,
            last_notified_at: None,
            last_saved_at: None,
            save_in_flight: false,
            csv_table_active: false,
            language_hint,
            pending_request_id: None,
            pending_request_at: None,
            next_request_id: 1,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.generation > self.last_saved_generation
    }

    fn allocate_request_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }
}

// ---------------------------------------------------------------------------
// Registry — managed Tauri state
// ---------------------------------------------------------------------------

/// Read-only snapshot for command handlers that need document metadata.
pub struct DocumentInfo {
    pub path: Option<PathBuf>,
    pub document_id: Option<String>,
    pub document_generation: Option<u64>,
    pub source: FileSource,
    pub is_dirty: bool,
    pub csv_table_active: bool,
    pub language_hint: String,
}

#[derive(Default)]
pub struct AutosaveRegistry {
    inner: Arc<Mutex<HashMap<String, AutosaveDocument>>>,
}

impl AutosaveRegistry {
    pub fn register(
        &self,
        window_label: &str,
        path: Option<PathBuf>,
        source: FileSource,
        language_hint: String,
    ) {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        map.insert(
            window_label.to_string(),
            AutosaveDocument::new(path, source, language_hint),
        );
    }

    pub fn register_authorized(
        &self,
        window_label: &str,
        path: PathBuf,
        source: FileSource,
        language_hint: String,
        document_id: String,
        document_generation: u64,
    ) {
        let mut document = AutosaveDocument::new(Some(path), source, language_hint);
        document.document_id = Some(document_id);
        document.document_generation = Some(document_generation);
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        map.insert(window_label.to_string(), document);
    }

    pub fn unregister(&self, window_label: &str) {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        map.remove(window_label);
    }

    pub fn notify_changed(&self, window_label: &str, generation: u64) {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(doc) = map.get_mut(window_label) {
            doc.generation = generation;
            doc.last_notified_at = Some(Instant::now());
        }
    }

    pub fn set_csv_mode(&self, window_label: &str, active: bool) {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(doc) = map.get_mut(window_label) {
            doc.csv_table_active = active;
        }
    }

    pub fn has_unsaved_changes(&self, window_label: &str) -> bool {
        let map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        map.get(window_label)
            .is_some_and(|doc| matches!(doc.source, FileSource::Slates) && doc.is_dirty())
    }

    /// Called by the timer thread to determine which documents need saving.
    pub fn check_and_trigger_saves(&self) -> Vec<SaveAction> {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let now = Instant::now();
        let mut actions = Vec::new();

        for (label, doc) in map.iter_mut() {
            // Only autosave slate files
            if !matches!(doc.source, FileSource::Slates) {
                continue;
            }

            // Check for content request timeout on in-flight saves
            if doc.save_in_flight {
                if let Some(request_at) = doc.pending_request_at {
                    if now.duration_since(request_at)
                        > Duration::from_millis(CONTENT_REQUEST_TIMEOUT_MS)
                    {
                        doc.save_in_flight = false;
                        doc.pending_request_id = None;
                        doc.pending_request_at = None;
                    }
                }
                continue;
            }

            if !doc.is_dirty() {
                continue;
            }

            let idle_ms = doc
                .last_notified_at
                .map(|t| now.duration_since(t).as_millis() as u64)
                .unwrap_or(0);

            let since_last_save_ms = doc
                .last_saved_at
                .map(|t| now.duration_since(t).as_millis() as u64)
                .unwrap_or(0); // New/untitled documents wait for the idle debounce first

            let should_save = idle_ms >= IDLE_DEBOUNCE_MS || since_last_save_ms >= MAX_LATENCY_MS;

            if should_save {
                if doc.csv_table_active {
                    actions.push(SaveAction::CsvDirect {
                        window_label: label.clone(),
                        path: doc.path.clone(),
                    });
                    doc.save_in_flight = true;
                } else {
                    let request_id = doc.allocate_request_id();
                    doc.pending_request_id = Some(request_id);
                    doc.pending_request_at = Some(now);
                    doc.save_in_flight = true;
                    actions.push(SaveAction::RequestContent {
                        window_label: label.clone(),
                        request_id,
                    });
                }
            }
        }

        actions
    }

    /// Mark a save as complete after content was successfully written.
    pub fn complete_save(&self, window_label: &str, saved_generation: u64) {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(doc) = map.get_mut(window_label) {
            if saved_generation > doc.last_saved_generation {
                doc.last_saved_generation = saved_generation;
            }
            doc.last_saved_at = Some(Instant::now());
            doc.save_in_flight = false;
            doc.pending_request_id = None;
            doc.pending_request_at = None;
        }
    }

    /// Clear the in-flight flag without updating saved generation (used on write failure).
    pub fn clear_in_flight(&self, window_label: &str) {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(doc) = map.get_mut(window_label) {
            doc.save_in_flight = false;
            doc.pending_request_id = None;
            doc.pending_request_at = None;
        }
    }

    pub fn validate_request(&self, window_label: &str, request_id: u64) -> bool {
        let map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        map.get(window_label)
            .is_some_and(|doc| doc.pending_request_id == Some(request_id))
    }

    pub fn get_document_info(&self, window_label: &str) -> Option<DocumentInfo> {
        let map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        map.get(window_label).map(|doc| DocumentInfo {
            path: doc.path.clone(),
            document_id: doc.document_id.clone(),
            document_generation: doc.document_generation,
            source: doc.source,
            is_dirty: doc.is_dirty(),
            csv_table_active: doc.csv_table_active,
            language_hint: doc.language_hint.clone(),
        })
    }

    pub fn update_path(&self, window_label: &str, path: PathBuf) {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(doc) = map.get_mut(window_label) {
            doc.path = Some(path);
        }
    }

    pub fn update_authorization(
        &self,
        window_label: &str,
        path: PathBuf,
        document_id: String,
        document_generation: u64,
    ) {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(doc) = map.get_mut(window_label) {
            doc.path = Some(path);
            doc.document_id = Some(document_id);
            doc.document_generation = Some(document_generation);
        }
    }

    pub fn update_authorization_if_matches(
        &self,
        window_label: &str,
        current_document_id: &str,
        path: PathBuf,
        document_generation: u64,
    ) {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(doc) = map.get_mut(window_label) {
            if doc.document_id.as_deref() == Some(current_document_id) {
                doc.path = Some(path);
                doc.document_generation = Some(document_generation);
            }
        }
    }

    pub fn update_language_hint(&self, window_label: &str, hint: &str) {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(doc) = map.get_mut(window_label) {
            doc.language_hint = hint.to_string();
        }
    }
}

// ---------------------------------------------------------------------------
// Save action types (returned by the timer check)
// ---------------------------------------------------------------------------

pub enum SaveAction {
    /// CSV table mode — serialize directly from CsvSession, no FE roundtrip.
    CsvDirect {
        window_label: String,
        path: Option<PathBuf>,
    },
    /// Text mode — emit an event to the FE requesting content.
    RequestContent {
        window_label: String,
        request_id: u64,
    },
}

// ---------------------------------------------------------------------------
// Atomic file write
// ---------------------------------------------------------------------------

const MAX_TEMP_FILE_ATTEMPTS: usize = 128;
static WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path_for(path: &Path, sequence: u64) -> PathBuf {
    let base = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "file".into());
    path.with_file_name(format!(".{}.{}.{}.tmp", base, std::process::id(), sequence))
}

fn create_unique_temp_file_with<N>(path: &Path, mut next_sequence: N) -> io::Result<(PathBuf, File)>
where
    N: FnMut() -> u64,
{
    for _ in 0..MAX_TEMP_FILE_ATTEMPTS {
        let temp_path = temp_path_for(path, next_sequence());
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;

            // Keep replacement contents private until the destination's
            // permissions have been copied immediately before replacement.
            options.mode(0o600);
        }

        match options.open(&temp_path) {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique temporary file",
    ))
}

#[cfg(not(windows))]
fn replace_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    std::fs::rename(temp_path, path)
}

#[cfg(not(windows))]
fn install_new_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    // `hard_link` fails when `path` already exists, giving new-document saves
    // atomic no-replace semantics that portable `rename` does not provide.
    std::fs::hard_link(temp_path, path)?;
    std::fs::remove_file(temp_path)
}

#[cfg(windows)]
fn install_new_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    use std::iter;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};

    let temp_wide: Vec<u16> = temp_path
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    let path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    // SAFETY: both buffers are owned, NUL-terminated UTF-16 strings and stay
    // alive for the call. Omitting MOVEFILE_REPLACE_EXISTING makes this fail
    // closed if another process creates the destination first.
    let moved = unsafe {
        MoveFileExW(
            temp_wide.as_ptr(),
            path_wide.as_ptr(),
            MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn replace_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    use std::iter;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let temp_wide: Vec<u16> = temp_path
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    let path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();

    // SAFETY: both buffers are owned, NUL-terminated UTF-16 strings and remain
    // alive for the duration of the call. The flags request same-volume
    // replacement and wait for the move to reach disk before returning.
    let replaced = unsafe {
        MoveFileExW(
            temp_wide.as_ptr(),
            path_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if replaced == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "save path has no parent"))?;
    File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn atomic_write_to_disk_with<N, W, R>(
    path: &Path,
    content: &[u8],
    next_sequence: N,
    write_content: W,
    replace: R,
) -> Result<(), String>
where
    N: FnMut() -> u64,
    W: FnOnce(&mut File, &[u8]) -> io::Result<()>,
    R: FnOnce(&Path, &Path) -> io::Result<()>,
{
    let existing_permissions = match std::fs::metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err("Save target exists and is not a regular file.".to_string());
            }
            Some(metadata.permissions())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("Failed to inspect save target: {}", error)),
    };

    let parent = path
        .parent()
        .ok_or_else(|| "Save path must have a parent directory.".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Failed to create directory: {}", error))?;

    let (temp_path, mut file) = create_unique_temp_file_with(path, next_sequence)
        .map_err(|error| format!("Failed to create temp file: {}", error))?;

    let write_result = write_content(&mut file, content)
        .and_then(|_| {
            if let Some(permissions) = existing_permissions {
                file.set_permissions(permissions)?;
            }
            Ok(())
        })
        .and_then(|_| file.sync_all());

    // Close the handle before cleanup or replacement. This is required on
    // Windows, where an open handle can prevent either operation.
    drop(file);

    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("Failed to write temp file: {}", error));
    }

    if let Err(error) = replace(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("Failed to replace file: {}", error));
    }

    sync_parent_directory(path).map_err(|error| format!("Failed to sync save directory: {}", error))
}

/// Durably write `content` to `path` via an exclusively-created temp file and
/// atomic same-directory replacement.
///
/// Shared by autosave and explicit saves (`write_file_content`). Properties:
/// - the temp file is hidden (leading `.`, so it never appears in the sidebar)
///   and opened with `create_new`, so stale files or attacker-created symlinks
///   are never followed or truncated;
/// - permissions from an existing destination are applied to the replacement;
/// - contents are flushed to disk (`sync_all`) before the rename, so a crash
///   between write and rename cannot leave a half-written destination;
/// - on rename failure the temp file is removed and the original file (if any)
///   is left intact.
pub fn autosave_write_to_disk(path: &Path, content: &str) -> Result<(), String> {
    use std::io::Write as _;

    atomic_write_to_disk_with(
        path,
        content.as_bytes(),
        || WRITE_COUNTER.fetch_add(1, Ordering::Relaxed),
        |file, bytes| file.write_all(bytes),
        replace_file,
    )
}

/// Durably create a brand-new document without ever replacing an existing
/// destination. The content is fully written and synced in a private temp file
/// before an atomic no-replace installation into the selected directory.
pub fn atomic_create_to_disk(path: &Path, content: &str) -> Result<(), String> {
    use std::io::Write as _;

    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => return Err("A file already exists at the new document path.".to_string()),
        Err(error) => return Err(format!("Failed to inspect new document path: {error}")),
    }

    let parent = path
        .parent()
        .ok_or_else(|| "Save path must have a parent directory.".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Failed to create directory: {error}"))?;
    let (temp_path, mut file) =
        create_unique_temp_file_with(path, || WRITE_COUNTER.fetch_add(1, Ordering::Relaxed))
            .map_err(|error| format!("Failed to create temp file: {error}"))?;

    let write_result = file
        .write_all(content.as_bytes())
        .and_then(|_| file.sync_all());
    drop(file);
    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("Failed to write temp file: {error}"));
    }
    if let Err(error) = install_new_file(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("Failed to create document: {error}"));
    }
    sync_parent_directory(path).map_err(|error| format!("Failed to sync save directory: {error}"))
}

// ---------------------------------------------------------------------------
// Background timer loop
// ---------------------------------------------------------------------------

/// Entry point for the background timer thread. Call from `setup()` via
/// `std::thread::spawn`.
pub fn run_timer_loop(app_handle: tauri::AppHandle) {
    let tick_duration = Duration::from_millis(TIMER_TICK_MS);

    loop {
        std::thread::sleep(tick_duration);

        let registry = app_handle.state::<AutosaveRegistry>();
        let actions = registry.check_and_trigger_saves();

        for action in actions {
            match action {
                SaveAction::CsvDirect { window_label, path } => {
                    handle_csv_direct_save(&app_handle, &window_label, path.as_deref());
                }
                SaveAction::RequestContent {
                    window_label,
                    request_id,
                } => {
                    if let Some(window) = app_handle.get_webview_window(&window_label) {
                        let payload = ContentRequestPayload { request_id };
                        if let Err(e) = window.emit(AUTOSAVE_REQUEST_CONTENT_EVENT, payload) {
                            eprintln!("Autosave: failed to emit content request: {}", e);
                            registry.clear_in_flight(&window_label);
                        }
                    } else {
                        registry.clear_in_flight(&window_label);
                    }
                }
            }
        }
    }
}

/// Serialize from CsvSession and write directly — no frontend roundtrip.
fn handle_csv_direct_save(app_handle: &tauri::AppHandle, window_label: &str, path: Option<&Path>) {
    let registry = app_handle.state::<AutosaveRegistry>();

    let path = match path {
        Some(p) => p.to_path_buf(),
        None => {
            // Cannot save CSV without a path (untitled document).
            // The naming pipeline requires text content which we could
            // generate from CSV, but that's a complex edge case.
            // Skip and let the next non-CSV save handle it.
            registry.clear_in_flight(window_label);
            return;
        }
    };

    let Some(document_info) = registry.get_document_info(window_label) else {
        registry.clear_in_flight(window_label);
        return;
    };
    let (Some(document_id), Some(document_generation)) = (
        document_info.document_id.as_deref(),
        document_info.document_generation,
    ) else {
        registry.clear_in_flight(window_label);
        return;
    };
    let documents = app_handle.state::<DocumentRegistry>();
    let storage = app_handle.state::<AppStorage>();
    let authorized = match documents.resolve(
        window_label,
        document_id,
        document_generation,
        DocumentAccess::Write,
    ) {
        Ok(document) => document,
        Err(error) => {
            eprintln!("Autosave: {error}");
            registry.clear_in_flight(window_label);
            return;
        }
    };
    if authorized.path != path
        || authorized.source != FileSource::Slates
        || revalidate_source_authority(app_handle, storage.inner(), &authorized).is_err()
    {
        eprintln!("Autosave: CSV document authorization changed before save");
        registry.clear_in_flight(window_label);
        return;
    }

    let csv_registry = app_handle.state::<CsvSessionRegistry>();
    let flush_result = csv_registry.try_flush_for_autosave(window_label);

    match flush_result {
        Some((version, content)) => match autosave_write_to_disk(&path, &content) {
            Ok(()) => {
                if let Err(error) = storage.record_file_update(&path, FileSource::Slates) {
                    eprintln!(
                        "Autosave: failed to update tracked-file metadata: {}",
                        error
                    );
                }
                let _ = app_handle.emit(RECENT_FILES_UPDATED_EVENT, "saved");
                registry.complete_save(window_label, version);
            }
            Err(e) => {
                eprintln!("{}", e);
                registry.clear_in_flight(window_label);
            }
        },
        None => {
            // No CSV session active — shouldn't happen since csv_table_active
            // was true, but handle gracefully.
            registry.clear_in_flight(window_label);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn test_autosave_write_to_disk_creates_file() {
        let dir = std::env::temp_dir().join("grayslate_autosave_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("test-file.txt");
        autosave_write_to_disk(&path, "hello world").unwrap();

        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content, "hello world");

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_autosave_write_to_disk_atomic_overwrites() {
        let dir = std::env::temp_dir().join("grayslate_autosave_test_overwrite");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("test-overwrite.txt");
        std::fs::write(&path, "original").unwrap();

        autosave_write_to_disk(&path, "updated").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "updated");

        // No temp file should remain in the directory after a successful write.
        let leftover_temps: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(
            leftover_temps.is_empty(),
            "temp files left behind: {:?}",
            leftover_temps
                .iter()
                .map(|e| e.file_name())
                .collect::<Vec<_>>()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_write_skips_preexisting_temp_path() {
        let dir = std::env::temp_dir().join("grayslate_atomic_temp_collision");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("document.txt");
        let collision = temp_path_for(&path, 41);
        std::fs::write(&collision, "do not touch").unwrap();
        let mut sequences = [41_u64, 42_u64].into_iter();

        atomic_write_to_disk_with(
            &path,
            b"saved",
            || sequences.next().unwrap(),
            |file, bytes| file.write_all(bytes),
            replace_file,
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "saved");
        assert_eq!(std::fs::read_to_string(&collision).unwrap(), "do not touch");
        assert!(!temp_path_for(&path, 42).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_atomic_write_never_follows_preexisting_temp_symlink() {
        use std::os::unix::fs::symlink;

        let dir = std::env::temp_dir().join("grayslate_atomic_temp_symlink");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("document.txt");
        let victim = dir.join("victim.txt");
        std::fs::write(&victim, "victim content").unwrap();
        let collision = temp_path_for(&path, 45);
        symlink(&victim, &collision).unwrap();
        let mut sequences = [45_u64, 46_u64].into_iter();

        atomic_write_to_disk_with(
            &path,
            b"saved",
            || sequences.next().unwrap(),
            |file, bytes| file.write_all(bytes),
            replace_file,
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "saved");
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "victim content");
        assert!(std::fs::symlink_metadata(&collision)
            .unwrap()
            .file_type()
            .is_symlink());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_write_failure_preserves_original_and_cleans_temp() {
        let dir = std::env::temp_dir().join("grayslate_atomic_write_failure");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("document.txt");
        std::fs::write(&path, "original").unwrap();
        let temp_path = temp_path_for(&path, 51);

        let result = atomic_write_to_disk_with(
            &path,
            b"replacement",
            || 51,
            |_file, _bytes| Err(io::Error::other("injected write failure")),
            replace_file,
        );

        assert!(result.unwrap_err().contains("injected write failure"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        assert!(!temp_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_replace_failure_preserves_original_and_cleans_temp() {
        let dir = std::env::temp_dir().join("grayslate_atomic_replace_failure");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("document.txt");
        std::fs::write(&path, "original").unwrap();
        let temp_path = temp_path_for(&path, 61);

        let result = atomic_write_to_disk_with(
            &path,
            b"replacement",
            || 61,
            |file, bytes| file.write_all(bytes),
            |_temp_path, _path| Err(io::Error::other("injected replace failure")),
        );

        assert!(result.unwrap_err().contains("injected replace failure"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        assert!(!temp_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_atomic_overwrite_preserves_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join("grayslate_atomic_permissions");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("document.txt");
        std::fs::write(&path, "original").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();

        autosave_write_to_disk(&path, "replacement").unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_create_refuses_to_replace_existing_destination() {
        let dir = std::env::temp_dir().join("grayslate_atomic_create_collision");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("document.txt");
        std::fs::write(&path, "original").unwrap();

        let result = atomic_create_to_disk(&path, "replacement");

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        let leftover_temps = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftover_temps, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_create_installs_complete_new_document() {
        let dir = std::env::temp_dir().join("grayslate_atomic_create_success");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("document.txt");

        atomic_create_to_disk(&path, "complete").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "complete");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_registry_generation_tracking() {
        let registry = AutosaveRegistry::default();
        registry.register(
            "main",
            Some(PathBuf::from("/tmp/test.md")),
            FileSource::Slates,
            "auto".into(),
        );

        assert!(!registry.has_unsaved_changes("main"));

        registry.notify_changed("main", 1);
        assert!(registry.has_unsaved_changes("main"));

        registry.complete_save("main", 1);
        assert!(!registry.has_unsaved_changes("main"));

        // More edits after save
        registry.notify_changed("main", 2);
        registry.notify_changed("main", 3);
        assert!(registry.has_unsaved_changes("main"));

        // Saving at gen 2 still leaves gen 3 dirty
        registry.complete_save("main", 2);
        assert!(registry.has_unsaved_changes("main"));

        registry.complete_save("main", 3);
        assert!(!registry.has_unsaved_changes("main"));
    }

    #[test]
    fn test_registry_local_files_not_autosaved() {
        let registry = AutosaveRegistry::default();
        registry.register(
            "main",
            Some(PathBuf::from("/tmp/local.txt")),
            FileSource::Local,
            "auto".into(),
        );

        registry.notify_changed("main", 1);
        // has_unsaved_changes only returns true for slates
        assert!(!registry.has_unsaved_changes("main"));
    }

    #[test]
    fn test_registry_unregister() {
        let registry = AutosaveRegistry::default();
        registry.register(
            "main",
            Some(PathBuf::from("/tmp/test.md")),
            FileSource::Slates,
            "auto".into(),
        );
        registry.notify_changed("main", 1);
        assert!(registry.has_unsaved_changes("main"));

        registry.unregister("main");
        assert!(!registry.has_unsaved_changes("main"));
    }

    #[test]
    fn test_document_info() {
        let registry = AutosaveRegistry::default();
        registry.register(
            "main",
            Some(PathBuf::from("/tmp/test.md")),
            FileSource::Slates,
            "python".into(),
        );
        registry.notify_changed("main", 5);

        let info = registry.get_document_info("main").unwrap();
        assert_eq!(info.path, Some(PathBuf::from("/tmp/test.md")));
        assert!(matches!(info.source, FileSource::Slates));
        assert!(info.is_dirty);
        assert!(!info.csv_table_active);
        assert_eq!(info.language_hint, "python");
    }

    #[test]
    fn test_csv_mode_toggle() {
        let registry = AutosaveRegistry::default();
        registry.register(
            "main",
            Some(PathBuf::from("/tmp/test.csv")),
            FileSource::Slates,
            "csv".into(),
        );

        let info = registry.get_document_info("main").unwrap();
        assert!(!info.csv_table_active);

        registry.set_csv_mode("main", true);
        let info = registry.get_document_info("main").unwrap();
        assert!(info.csv_table_active);

        registry.set_csv_mode("main", false);
        let info = registry.get_document_info("main").unwrap();
        assert!(!info.csv_table_active);
    }

    #[test]
    fn test_validate_request() {
        let registry = AutosaveRegistry::default();
        registry.register(
            "main",
            Some(PathBuf::from("/tmp/test.md")),
            FileSource::Slates,
            "auto".into(),
        );

        // No pending request
        assert!(!registry.validate_request("main", 1));

        // Simulate the timer setting a pending request
        {
            let mut map = registry.inner.lock().unwrap();
            let doc = map.get_mut("main").unwrap();
            let id = doc.allocate_request_id();
            doc.pending_request_id = Some(id);
            assert_eq!(id, 1);
        }

        assert!(registry.validate_request("main", 1));
        assert!(!registry.validate_request("main", 2));
        assert!(!registry.validate_request("main", 0));
    }

    #[test]
    fn test_update_path() {
        let registry = AutosaveRegistry::default();
        registry.register("main", None, FileSource::Slates, "auto".into());

        let info = registry.get_document_info("main").unwrap();
        assert!(info.path.is_none());

        registry.update_path("main", PathBuf::from("/tmp/new-slate.md"));

        let info = registry.get_document_info("main").unwrap();
        assert_eq!(info.path, Some(PathBuf::from("/tmp/new-slate.md")));
    }

    #[test]
    fn test_check_triggers_only_for_idle_slates() {
        let registry = AutosaveRegistry::default();

        // Register a slate that's been idle long enough
        registry.register(
            "main",
            Some(PathBuf::from("/tmp/test.md")),
            FileSource::Slates,
            "auto".into(),
        );
        registry.notify_changed("main", 1);

        // Backdate the notification to make it appear idle
        {
            let mut map = registry.inner.lock().unwrap();
            let doc = map.get_mut("main").unwrap();
            doc.last_notified_at = Some(Instant::now() - Duration::from_secs(5));
        }

        let actions = registry.check_and_trigger_saves();
        assert_eq!(actions.len(), 1);
        assert!(
            matches!(&actions[0], SaveAction::RequestContent { window_label, request_id: 1 } if window_label == "main")
        );
    }

    #[test]
    fn test_new_untitled_slate_waits_for_idle_debounce() {
        let registry = AutosaveRegistry::default();

        // A brand-new untitled slate has never been saved and just received
        // its first keystroke. It should NOT be named/saved until the user
        // pauses long enough to exceed IDLE_DEBOUNCE_MS.
        registry.register("main", None, FileSource::Slates, "auto".into());
        registry.notify_changed("main", 1);

        let actions = registry.check_and_trigger_saves();
        assert!(
            actions.is_empty(),
            "recently edited untitled slate should wait for idle debounce"
        );

        // Simulate the user having stopped typing for longer than the debounce.
        {
            let mut map = registry.inner.lock().unwrap();
            let doc = map.get_mut("main").unwrap();
            doc.last_notified_at = Some(Instant::now() - Duration::from_secs(5));
        }

        let actions = registry.check_and_trigger_saves();
        assert_eq!(actions.len(), 1);
        assert!(
            matches!(&actions[0], SaveAction::RequestContent { window_label, request_id: 1 } if window_label == "main"),
            "untitled slate should trigger once idle debounce expires"
        );
    }

    #[test]
    fn test_check_skips_local_files() {
        let registry = AutosaveRegistry::default();
        registry.register(
            "main",
            Some(PathBuf::from("/tmp/local.txt")),
            FileSource::Local,
            "auto".into(),
        );
        registry.notify_changed("main", 1);

        {
            let mut map = registry.inner.lock().unwrap();
            let doc = map.get_mut("main").unwrap();
            doc.last_notified_at = Some(Instant::now() - Duration::from_secs(5));
        }

        let actions = registry.check_and_trigger_saves();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_respects_in_flight() {
        let registry = AutosaveRegistry::default();
        registry.register(
            "main",
            Some(PathBuf::from("/tmp/test.md")),
            FileSource::Slates,
            "auto".into(),
        );
        registry.notify_changed("main", 1);

        {
            let mut map = registry.inner.lock().unwrap();
            let doc = map.get_mut("main").unwrap();
            doc.last_notified_at = Some(Instant::now() - Duration::from_secs(5));
            doc.save_in_flight = true;
            doc.pending_request_at = Some(Instant::now());
        }

        let actions = registry.check_and_trigger_saves();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_check_csv_mode_triggers_direct_save() {
        let registry = AutosaveRegistry::default();
        registry.register(
            "main",
            Some(PathBuf::from("/tmp/test.csv")),
            FileSource::Slates,
            "csv".into(),
        );
        registry.set_csv_mode("main", true);
        registry.notify_changed("main", 1);

        {
            let mut map = registry.inner.lock().unwrap();
            let doc = map.get_mut("main").unwrap();
            doc.last_notified_at = Some(Instant::now() - Duration::from_secs(5));
        }

        let actions = registry.check_and_trigger_saves();
        assert_eq!(actions.len(), 1);
        assert!(
            matches!(&actions[0], SaveAction::CsvDirect { window_label, .. } if window_label == "main")
        );
    }

    #[test]
    fn test_content_request_timeout_resets_in_flight() {
        let registry = AutosaveRegistry::default();
        registry.register(
            "main",
            Some(PathBuf::from("/tmp/test.md")),
            FileSource::Slates,
            "auto".into(),
        );
        registry.notify_changed("main", 1);

        // Simulate an in-flight request that timed out
        {
            let mut map = registry.inner.lock().unwrap();
            let doc = map.get_mut("main").unwrap();
            doc.save_in_flight = true;
            doc.pending_request_id = Some(1);
            doc.pending_request_at = Some(Instant::now() - Duration::from_secs(10));
            doc.last_notified_at = Some(Instant::now() - Duration::from_secs(10));
        }

        // First check: timeout clears in_flight
        let actions = registry.check_and_trigger_saves();
        // The timeout clear and the new trigger happen in separate iterations
        // because the continue after clearing skips the rest of the loop body.
        assert!(actions.is_empty());

        // Second check: now it should trigger
        let actions = registry.check_and_trigger_saves();
        assert_eq!(actions.len(), 1);
    }
}
