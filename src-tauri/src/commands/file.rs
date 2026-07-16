use std::{
    collections::HashMap,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::document::{
    canonical_notes_root, classify_existing_document, classify_new_document, open_authorized_read,
    revalidate_source_authority, AuthorizedDocument, DocumentAccess, DocumentDescriptor,
    DocumentRegistry, DocumentRights,
};
use crate::filesystem::{
    resolve_default_notes_root_path, resolve_notes_root_path, sanitize_filename,
    unique_path_in_dir_excluding,
};
use crate::storage::{
    normalize_path_key, AppStorage, FileSource, RecentFileRecord, SETTING_CONFIRM_BEFORE_DELETE,
    SETTING_DEFAULT_INDENT_MODE, SETTING_DEFAULT_INDENT_SIZE, SETTING_FONT_SIZE,
    SETTING_LAST_ACTIVE_FILE, SETTING_NOTES_ROOT, SETTING_SIDEBAR_OPEN, SETTING_SIDEBAR_WIDTH,
    SETTING_STARTUP_BEHAVIOR, SETTING_THEME, SETTING_WORD_WRAP,
};

use super::RECENT_FILES_UPDATED_EVENT;

/// Maximum file size allowed to be opened: 200 MB.
const MAX_FILE_SIZE: u64 = 200 * 1024 * 1024;
const FILE_READ_CHUNK_SIZE: usize = 256 * 1024;
const FILE_READ_CANCELLED_MESSAGE: &str = "File read cancelled.";
const MAX_RECENT_FILES_LIMIT: usize = 200;

#[derive(serde::Serialize)]
pub struct AuthorizedRecentFileRecord {
    #[serde(flatten)]
    pub file: RecentFileRecord,
    pub document_id: String,
    pub document_generation: u64,
}

fn source_from_record(record: &RecentFileRecord) -> Result<FileSource, String> {
    match record.source.as_str() {
        "slates" => Ok(FileSource::Slates),
        "local" => Ok(FileSource::Local),
        _ => Err("Tracked file has an invalid source classification.".to_string()),
    }
}

fn grant_tracked_record(
    app: &tauri::AppHandle,
    storage: &AppStorage,
    documents: &DocumentRegistry,
    window_label: &str,
    record: RecentFileRecord,
) -> Result<AuthorizedRecentFileRecord, String> {
    let recorded_source = source_from_record(&record)?;
    let path = PathBuf::from(&record.path);
    let (canonical, actual_source) = classify_existing_document(app, storage, &path)?;
    if recorded_source != actual_source {
        return Err("Tracked file no longer matches its authorized source.".to_string());
    }
    let granted = documents.grant_existing(
        window_label,
        &canonical,
        actual_source,
        DocumentRights::tracked(actual_source),
    )?;
    Ok(AuthorizedRecentFileRecord {
        file: record,
        document_id: granted.id,
        document_generation: granted.generation,
    })
}

fn resolve_document(
    app: &tauri::AppHandle,
    storage: &AppStorage,
    documents: &DocumentRegistry,
    window_label: &str,
    document_id: &str,
    document_generation: u64,
    access: DocumentAccess,
) -> Result<AuthorizedDocument, String> {
    let document = documents.resolve(window_label, document_id, document_generation, access)?;
    revalidate_source_authority(app, storage, &document)?;
    Ok(document)
}

fn require_tracked_document(
    app: &tauri::AppHandle,
    storage: &AppStorage,
    documents: &DocumentRegistry,
    window_label: &str,
    document_id: &str,
    document_generation: u64,
    access: DocumentAccess,
) -> Result<AuthorizedDocument, String> {
    let document = resolve_document(
        app,
        storage,
        documents,
        window_label,
        document_id,
        document_generation,
        access,
    )?;
    let record = storage
        .get_tracked_file(&document.path)?
        .ok_or_else(|| "Document is not tracked by Grayslate.".to_string())?;
    if source_from_record(&record)? != document.source {
        return Err("Tracked document source does not match its grant.".to_string());
    }
    Ok(document)
}

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

    let file = open_authorized_read(path)?;
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

        if bytes.len().saturating_add(bytes_read) > MAX_FILE_SIZE as usize {
            return Err(format!(
                "File exceeds the maximum supported size of {} MB",
                MAX_FILE_SIZE / (1024 * 1024)
            ));
        }

        bytes.extend_from_slice(&chunk[..bytes_read]);
    }

    ensure_read_not_cancelled(cancelled)?;

    // Validate UTF-8 without converting to String — avoids an allocation.
    std::str::from_utf8(&bytes).map_err(|error| format!("Failed to read file: {}", error))?;

    Ok(bytes)
}

/// Reject paths whose target is not a regular file.
///
/// `metadata` follows symlinks, so a symlink pointing at a regular file passes.
/// This blocks opening/overwriting directories, device nodes, FIFOs, and sockets
/// — none of which are meaningful in a text scratchpad, and some of which
/// (FIFOs, block devices) would otherwise stall the read. This file-type check
/// is defense in depth only: CSP does not authorize privileged IPC, and the
/// pending Rust-owned document-grant work remains the actual path authority
/// boundary.
fn ensure_regular_file(metadata: &std::fs::Metadata) -> Result<(), String> {
    if !metadata.is_file() {
        return Err("Path is not a regular file.".to_string());
    }
    Ok(())
}

/// Best-effort resolution of the current user's home directory without pulling
/// in a Tauri handle. Used only to reject a home-directory notes root.
fn user_home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// Reject notes-root choices that would make Grayslate treat sweeping parts of
/// the filesystem as managed "slate" files.
///
/// The notes root defines which files count as `FileSource::Slates`, and the
/// slate-only gate (`require_slate_file`) authorizes rename/delete. A too-broad
/// root (a filesystem root, or the whole home directory) would turn that gate
/// into near-arbitrary rename/delete authority, so we refuse those. The path is
/// canonicalized first so a symlinked or `..`-traversal root cannot slip past
/// these checks.
fn validate_notes_root_choice(path: &Path) -> Result<(), String> {
    let resolved = match std::fs::canonicalize(path) {
        Ok(canonical) => {
            if !canonical.is_dir() {
                return Err("Notes root must be a directory.".to_string());
            }
            canonical
        }
        Err(_) => {
            // The directory does not exist yet — validate the parent (which
            // must exist) so the root can be created safely under it.
            let parent = path
                .parent()
                .ok_or_else(|| "Notes root cannot be a filesystem root.".to_string())?;
            let parent_canonical = std::fs::canonicalize(parent)
                .map_err(|_| "Notes root parent directory does not exist.".to_string())?;
            let leaf = path
                .file_name()
                .ok_or_else(|| "Notes root must name a directory.".to_string())?;
            parent_canonical.join(leaf)
        }
    };

    // A filesystem root ("/", "C:\\", …) has no parent.
    if resolved.parent().is_none() {
        return Err("Notes root cannot be a filesystem root.".to_string());
    }

    // The home directory itself is too broad; a subfolder of it is fine.
    if let Some(home) = user_home_dir() {
        if let Ok(home_canonical) = std::fs::canonicalize(&home) {
            if resolved == home_canonical {
                return Err(
                    "Notes root cannot be your home directory itself; choose a subfolder."
                        .to_string(),
                );
            }
        }
    }

    Ok(())
}

fn validate_last_active_file_choice(value: &str) -> Result<(), String> {
    if value.len() > 32 * 1024 {
        return Err("Last active file path is too long.".to_string());
    }
    if value.contains('\0') {
        return Err("Last active file path contains an invalid character.".to_string());
    }

    let path = Path::new(value);
    if !path.is_absolute() {
        return Err("Last active file path must be absolute.".to_string());
    }

    match std::fs::metadata(path) {
        Ok(metadata) => ensure_regular_file(&metadata)
            .map_err(|_| "Last active file path must identify a regular file.".to_string()),
        // The last-opened file may legitimately have been moved or deleted
        // between sessions. Startup revalidates it before attempting a read.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Cannot validate last active file path: {}", error)),
    }
}

#[cfg(test)]
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

    // If the target already exists, refuse to overwrite anything that is not a
    // regular file (e.g. a directory or device node). A not-yet-existing path
    // is fine — that is a normal new-file save.
    if let Ok(metadata) = std::fs::metadata(path) {
        ensure_regular_file(&metadata)
            .map_err(|_| "Save target exists and is not a regular file.".to_string())?;
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
    documents: tauri::State<'_, DocumentRegistry>,
    cancellations: tauri::State<'_, FileReadCancellationRegistry>,
    window: tauri::Window,
    document_id: String,
    document_generation: u64,
    request_id: u64,
) -> Result<tauri::ipc::Response, String> {
    let window_label = window.label().to_string();
    let cancellation_flag = cancellations.begin_request(&window_label, request_id);

    let result = async {
        let document = resolve_document(
            &app,
            storage.inner(),
            documents.inner(),
            &window_label,
            &document_id,
            document_generation,
            DocumentAccess::Read,
        )?;
        let metadata = std::fs::metadata(&document.path)
            .map_err(|error| format!("Cannot access file: {}", error))?;

        // Only regular files may be opened. Grant resolution already rejects
        // symlinks; this second check rejects directories, device nodes, FIFOs,
        // and sockets before the blocking read is attempted.
        ensure_regular_file(&metadata)?;

        ensure_read_not_cancelled(cancellation_flag.as_ref())?;

        if metadata.len() > MAX_FILE_SIZE {
            let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
            return Err(format!(
                "File is too large ({:.1} MB). The maximum allowed size is 200 MB.",
                size_mb
            ));
        }

        let read_path = document.path.clone();
        let read_cancelled = Arc::clone(&cancellation_flag);
        let bytes = tauri::async_runtime::spawn_blocking(move || {
            read_file_bytes_cancellable(&read_path, read_cancelled.as_ref())
        })
        .await
        .map_err(|error| format!("Failed to join file read task: {}", error))??;

        ensure_read_not_cancelled(cancellation_flag.as_ref())?;

        if storage.record_file_open_if_untracked(&document.path, document.source)? {
            let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());
        }
        Ok(tauri::ipc::Response::new(bytes))
    }
    .await;

    cancellations.finish_request(&window_label, request_id);

    result
}

#[tauri::command]
pub async fn pick_document(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
) -> Result<Option<DocumentDescriptor>, String> {
    let dialog_app = app.clone();
    let dialog_window = window.clone();
    let selected = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .set_parent(&dialog_window)
            .blocking_pick_file()
    })
    .await
    .map_err(|error| format!("Failed to join open dialog task: {error}"))?;

    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| format!("Selected file is not a filesystem path: {error}"))?;
    let (canonical, source) = classify_existing_document(&app, storage.inner(), &path)?;
    let granted = documents.grant_existing(
        window.label(),
        &canonical,
        source,
        DocumentRights::tracked(source),
    )?;
    Ok(Some(granted.descriptor()))
}

#[tauri::command]
pub async fn pick_save_document(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    current_document_id: Option<String>,
    current_document_generation: Option<u64>,
    suggested_name: Option<String>,
) -> Result<Option<DocumentDescriptor>, String> {
    let mut dialog = app.dialog().file().set_parent(&window).set_title("Save As");

    if let (Some(id), Some(generation)) =
        (current_document_id.as_deref(), current_document_generation)
    {
        let current = resolve_document(
            &app,
            storage.inner(),
            documents.inner(),
            window.label(),
            id,
            generation,
            DocumentAccess::Read,
        )?;
        if let Some(parent) = current.path.parent() {
            dialog = dialog.set_directory(parent);
        }
        if let Some(name) = current.path.file_name() {
            dialog = dialog.set_file_name(name.to_string_lossy());
        }
    } else {
        let root = canonical_notes_root(&app, storage.inner(), true)?;
        dialog = dialog.set_directory(root);
        if let Some(name) = suggested_name {
            let safe_name = sanitize_filename(&name);
            validate_new_filename(&safe_name)?;
            dialog = dialog.set_file_name(safe_name);
        }
    }

    let selected = tauri::async_runtime::spawn_blocking(move || dialog.blocking_save_file())
        .await
        .map_err(|error| format!("Failed to join save dialog task: {error}"))?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| format!("Selected file is not a filesystem path: {error}"))?;

    let (authorized_path, source, exists) = if path.exists() {
        let (canonical, source) = classify_existing_document(&app, storage.inner(), &path)?;
        (canonical, source, true)
    } else {
        let (candidate, source) = classify_new_document(&app, storage.inner(), &path)?;
        (candidate, source, false)
    };
    let granted = if exists {
        documents.grant_existing(
            window.label(),
            &authorized_path,
            source,
            DocumentRights::tracked(source),
        )?
    } else {
        documents.grant_new(
            window.label(),
            &authorized_path,
            source,
            DocumentRights::tracked(source),
        )?
    };
    Ok(Some(granted.descriptor()))
}

#[tauri::command]
pub fn cancel_file_read(
    cancellations: tauri::State<'_, FileReadCancellationRegistry>,
    window: tauri::Window,
) {
    cancellations.cancel_window_request(window.label());
}

#[tauri::command]
pub fn reveal_document(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    document_id: String,
    document_generation: u64,
) -> Result<(), String> {
    let document = resolve_document(
        &app,
        storage.inner(),
        documents.inner(),
        window.label(),
        &document_id,
        document_generation,
        DocumentAccess::Read,
    )?;
    app.opener()
        .reveal_item_in_dir(&document.path)
        .map_err(|error| format!("Failed to reveal document: {error}"))
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
pub async fn pick_notes_root(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    window: tauri::Window,
) -> Result<Option<String>, String> {
    let dialog_app = app.clone();
    let dialog_window = window.clone();
    let selected = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .set_parent(&dialog_window)
            .set_title("Choose Grayslate notes folder")
            .blocking_pick_folder()
    })
    .await
    .map_err(|error| format!("Failed to join notes-folder dialog task: {error}"))?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| format!("Selected folder is not a filesystem path: {error}"))?;
    validate_notes_root_choice(&path)?;
    let canonical = std::fs::canonicalize(path)
        .map_err(|error| format!("Cannot resolve selected notes folder: {error}"))?;
    let value = canonical.to_string_lossy().into_owned();
    storage.set_setting(SETTING_NOTES_ROOT, Some(&value))?;
    Ok(Some(value))
}

#[tauri::command]
pub fn reset_notes_root(storage: tauri::State<'_, AppStorage>) -> Result<(), String> {
    storage.set_setting(SETTING_NOTES_ROOT, None)
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
pub fn get_last_active_document(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
) -> Result<Option<DocumentDescriptor>, String> {
    let Some(path) = storage.get_setting(SETTING_LAST_ACTIVE_FILE)? else {
        return Ok(None);
    };
    let path = PathBuf::from(path);
    let Some(record) = storage.get_tracked_file(&path)? else {
        return Ok(None);
    };
    let authorized = grant_tracked_record(
        &app,
        storage.inner(),
        documents.inner(),
        window.label(),
        record,
    )?;
    let document = documents.resolve(
        window.label(),
        &authorized.document_id,
        authorized.document_generation,
        DocumentAccess::Read,
    )?;
    Ok(Some(document.descriptor()))
}

#[tauri::command]
pub fn set_last_active_document(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    document_id: Option<String>,
    document_generation: Option<u64>,
) -> Result<(), String> {
    match (document_id.as_deref(), document_generation) {
        (None, None) => storage.set_setting(SETTING_LAST_ACTIVE_FILE, None),
        (Some(id), Some(generation)) => {
            let document = resolve_document(
                &app,
                storage.inner(),
                documents.inner(),
                window.label(),
                id,
                generation,
                DocumentAccess::Read,
            )?;
            let path = document.path.to_string_lossy();
            storage.set_setting(SETTING_LAST_ACTIVE_FILE, Some(path.as_ref()))
        }
        _ => Err("Document ID and generation must be provided together.".to_string()),
    }
}

#[tauri::command]
pub fn set_app_setting(
    storage: tauri::State<'_, AppStorage>,
    key: String,
    value: Option<String>,
) -> Result<(), String> {
    match key.as_str() {
        SETTING_NOTES_ROOT => {
            return Err(
                "Notes root can only be changed through the native folder picker.".to_string(),
            );
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
                let parsed: i32 = size
                    .parse()
                    .map_err(|_| format!("Font size must be a number, got \"{}\".", size))?;
                if !(10..=24).contains(&parsed) {
                    return Err(format!(
                        "Font size must be between 10 and 24, got {}.",
                        parsed
                    ));
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
                let parsed: i32 = width
                    .parse()
                    .map_err(|_| format!("Sidebar width must be a number, got \"{}\".", width))?;
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
        SETTING_STARTUP_BEHAVIOR => {
            if let Some(ref behavior) = value {
                if behavior != "new" && behavior != "last" {
                    return Err("Startup behavior must be \"new\" or \"last\".".to_string());
                }
            }
        }
        SETTING_DEFAULT_INDENT_MODE => {
            if let Some(ref mode) = value {
                if mode != "spaces" && mode != "tab" {
                    return Err("Default indent mode must be \"spaces\" or \"tab\".".to_string());
                }
            }
        }
        SETTING_DEFAULT_INDENT_SIZE => {
            if let Some(ref size) = value {
                let parsed: i32 = size.parse().map_err(|_| {
                    format!("Default indent size must be a number, got \"{}\".", size)
                })?;
                if !(1..=8).contains(&parsed) {
                    return Err(format!(
                        "Default indent size must be between 1 and 8, got {}.",
                        parsed
                    ));
                }
            }
        }
        SETTING_CONFIRM_BEFORE_DELETE => {
            if let Some(ref confirm) = value {
                if confirm != "true" && confirm != "false" {
                    return Err("Confirm before delete must be \"true\" or \"false\".".to_string());
                }
            }
        }
        // Internal bookkeeping path (or None to clear). It may be stale by the
        // next launch, but it must still have the shape of a file path written
        // by the editor rather than an arbitrary setting payload.
        SETTING_LAST_ACTIVE_FILE => {
            if let Some(ref last_active_file) = value {
                validate_last_active_file_choice(last_active_file)?;
            }
        }
        // Reject any key the backend does not recognize rather than silently
        // persisting attacker- or bug-supplied settings.
        other => {
            return Err(format!("Unknown setting key: {}", other));
        }
    }

    storage.set_setting(&key, value.as_deref())
}

#[tauri::command]
pub fn get_recent_files(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    limit: Option<usize>,
) -> Result<Vec<AuthorizedRecentFileRecord>, String> {
    let limit = clamp_recent_files_limit(limit);

    // Keep the tracking table in sync with the filesystem before returning
    // results so newly-added / externally-deleted slates files are reflected
    // in the sidebar immediately.
    sync_notes_tracking(&app, storage.inner());

    // Refresh metadata for every tracked file before the final query so
    // disk-side changes (including local files outside the notes root) are
    // reflected in the ordering. With the conditional-update logic in
    // refresh_tracked_file this is a no-op for unchanged rows.
    let tracked = storage.list_tracked_files()?;
    for file in &tracked {
        let path = PathBuf::from(&file.path);
        if let Ok((canonical, source)) = classify_existing_document(&app, storage.inner(), &path) {
            storage.refresh_tracked_file(&canonical, source)?;
        }
    }

    // Prune entries that no longer exist on disk so the sidebar stays clean.
    let rows = storage.list_recent_files(limit)?;
    let mut result = Vec::new();
    for file in rows {
        let path = PathBuf::from(&file.path);
        if path.exists() {
            match grant_tracked_record(
                &app,
                storage.inner(),
                documents.inner(),
                window.label(),
                file,
            ) {
                Ok(authorized) => result.push(authorized),
                Err(error) => eprintln!("Skipping unauthorized recent file: {error}"),
            }
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
/// Only touches rows whose `source` is `'slates'` — local files
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
                path, e
            );
        }
    }
}

#[tauri::command]
pub async fn write_file_content(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    autosave: tauri::State<'_, crate::autosave::AutosaveRegistry>,
    window: tauri::Window,
    document_id: String,
    document_generation: u64,
    content: String,
) -> Result<DocumentDescriptor, String> {
    let document = resolve_document(
        &app,
        storage.inner(),
        documents.inner(),
        window.label(),
        &document_id,
        document_generation,
        DocumentAccess::Write,
    )?;
    let target_path = document.path.clone();

    // Durable, atomic save via the shared temp-file + rename helper so a crash
    // mid-write cannot truncate or corrupt the destination file.
    let path_for_write = target_path.clone();
    let is_new_document = !document.exists;
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        if is_new_document {
            crate::autosave::atomic_create_to_disk(&path_for_write, &content)
        } else {
            crate::autosave::autosave_write_to_disk(&path_for_write, &content)
        }
    })
    .await
    .map_err(|error| format!("Failed to join file write task: {}", error))??;

    let saved = documents.mark_created(window.label(), &document_id, document_generation)?;
    revalidate_source_authority(&app, storage.inner(), &saved)?;
    storage.record_file_update(&target_path, saved.source)?;
    autosave.register_authorized(
        window.label(),
        saved.path.clone(),
        saved.source,
        "auto".to_string(),
        saved.id.clone(),
        saved.generation,
    );
    let _ = app.emit(RECENT_FILES_UPDATED_EVENT, "saved");
    Ok(saved.descriptor())
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

fn copy_file_create_new(source: &Path, destination: &Path) -> Result<(), String> {
    let mut input = open_authorized_read(source)?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| format!("Failed to create duplicate: {error}"))?;
    if let Err(error) = std::io::copy(&mut input, &mut output) {
        drop(output);
        let _ = std::fs::remove_file(destination);
        return Err(format!("Failed to copy file: {error}"));
    }
    if let Err(error) = output.sync_all() {
        drop(output);
        let _ = std::fs::remove_file(destination);
        return Err(format!("Failed to flush duplicate: {error}"));
    }
    Ok(())
}

#[cfg(unix)]
fn rename_file_no_replace(source: &Path, destination: &Path) -> Result<(), String> {
    std::fs::hard_link(source, destination)
        .map_err(|error| format!("Failed to reserve renamed file: {error}"))?;
    if let Err(error) = std::fs::remove_file(source) {
        let _ = std::fs::remove_file(destination);
        return Err(format!("Failed to remove old file after rename: {error}"));
    }
    Ok(())
}

#[cfg(not(unix))]
fn rename_file_no_replace(source: &Path, destination: &Path) -> Result<(), String> {
    // Windows rename fails if the destination exists, providing the required
    // no-replace behavior for the collision-free name selected above.
    std::fs::rename(source, destination).map_err(|error| format!("Failed to rename file: {error}"))
}

/// Remove a local file from sidebar tracking without deleting
/// it from disk.  Returns an error if the path is a managed slate file.
#[tauri::command]
pub fn untrack_local_file(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    document_id: String,
    document_generation: u64,
) -> Result<(), String> {
    let document = require_tracked_document(
        &app,
        storage.inner(),
        documents.inner(),
        window.label(),
        &document_id,
        document_generation,
        DocumentAccess::Read,
    )?;
    if document.source == FileSource::Slates {
        return Err("Cannot untrack a Grayslate slate file. Use Delete instead.".to_string());
    }

    storage.delete_tracked_file(&document.path)?;
    documents.revoke(window.label(), &document_id);
    let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());
    Ok(())
}

/// Permanently delete a slate file from disk and remove it from tracking.
/// Returns an error if `path` is not a managed slate file.
#[tauri::command]
pub async fn delete_file(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    document_id: String,
    document_generation: u64,
) -> Result<(), String> {
    let document = require_tracked_document(
        &app,
        storage.inner(),
        documents.inner(),
        window.label(),
        &document_id,
        document_generation,
        DocumentAccess::Manage,
    )?;
    if document.source != FileSource::Slates {
        return Err("Delete is only available for managed slate files.".to_string());
    }
    let target = document.path;

    let target_clone = target.clone();
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::remove_file(&target_clone).map_err(|e| format!("Failed to delete file: {}", e))
    })
    .await
    .map_err(|e| format!("Delete task failed: {}", e))??;

    // Best-effort removal from tracking; ignore errors if the row was never
    // stored.
    let _ = storage.delete_tracked_file(&target);
    documents.revoke(window.label(), &document_id);
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
    documents: tauri::State<'_, DocumentRegistry>,
    autosave: tauri::State<'_, crate::autosave::AutosaveRegistry>,
    window: tauri::Window,
    document_id: String,
    document_generation: u64,
    new_name: String,
) -> Result<DocumentDescriptor, String> {
    let document = require_tracked_document(
        &app,
        storage.inner(),
        documents.inner(),
        window.label(),
        &document_id,
        document_generation,
        DocumentAccess::Manage,
    )?;
    if document.source != FileSource::Slates {
        return Err("Rename is only available for managed slate files.".to_string());
    }
    let old_path = document.path.clone();

    let sanitized_name = sanitize_filename(&new_name);
    validate_new_filename(&sanitized_name)?;

    let parent = old_path
        .parent()
        .ok_or_else(|| "File has no parent directory.".to_string())?
        .to_path_buf();

    let new_path = unique_path_in_dir_excluding(&parent, &sanitized_name, Some(&old_path));
    // A submitted unchanged filename resolves to the source path. There is
    // nothing to rename, and attempting fs::rename(source, source) fails on
    // some platforms.
    if new_path == old_path {
        return Ok(document.descriptor());
    }

    let old_clone = old_path.clone();
    let new_clone = new_path.clone();
    tauri::async_runtime::spawn_blocking(move || rename_file_no_replace(&old_clone, &new_clone))
        .await
        .map_err(|e| format!("Rename task failed: {}", e))??;

    storage.rename_tracked_file(&old_path, &new_path)?;
    let renamed =
        documents.replace_path(window.label(), &document_id, document_generation, &new_path)?;
    autosave.update_authorization_if_matches(
        window.label(),
        &document_id,
        renamed.path.clone(),
        renamed.generation,
    );
    let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());

    Ok(renamed.descriptor())
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
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    document_id: String,
    document_generation: u64,
) -> Result<DocumentDescriptor, String> {
    let document = require_tracked_document(
        &app,
        storage.inner(),
        documents.inner(),
        window.label(),
        &document_id,
        document_generation,
        DocumentAccess::Read,
    )?;
    if document.source != FileSource::Local {
        return Err("Only local files can be duplicated into slates.".to_string());
    }
    let src = document.path;

    let src_name = src
        .file_name()
        .ok_or_else(|| "File has no name.".to_string())?
        .to_string_lossy()
        .to_string();

    let notes_root = canonical_notes_root(&app, storage.inner(), true)?;

    let copy_name = make_copy_name(&src_name);
    let dest = next_copy_path_in_dir(&notes_root, &copy_name);
    let granted = documents.grant_new(
        window.label(),
        &dest,
        FileSource::Slates,
        DocumentRights::tracked(FileSource::Slates),
    )?;

    let src_clone = src.clone();
    let dest_clone = dest.clone();
    tauri::async_runtime::spawn_blocking(move || copy_file_create_new(&src_clone, &dest_clone))
        .await
        .map_err(|e| format!("Duplicate task failed: {}", e))??;

    storage.record_file_update(&dest, crate::storage::FileSource::Slates)?;
    let created = documents.mark_created(window.label(), &granted.id, granted.generation)?;
    let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());

    Ok(created.descriptor())
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
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    document_id: String,
    document_generation: u64,
) -> Result<DocumentDescriptor, String> {
    let document = require_tracked_document(
        &app,
        storage.inner(),
        documents.inner(),
        window.label(),
        &document_id,
        document_generation,
        DocumentAccess::Read,
    )?;
    let src = document.path;

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
    let granted = documents.grant_new(
        window.label(),
        &dest,
        document.source,
        DocumentRights::tracked(document.source),
    )?;

    let src_clone = src.clone();
    let dest_clone = dest.clone();
    tauri::async_runtime::spawn_blocking(move || copy_file_create_new(&src_clone, &dest_clone))
        .await
        .map_err(|e| format!("Duplicate task failed: {}", e))??;

    storage.record_file_update(&dest, document.source)?;
    let created = documents.mark_created(window.label(), &granted.id, granted.generation)?;
    let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());

    Ok(created.descriptor())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn write_path_rejects_relative() {
        assert!(validate_write_path(Path::new("relative/file.txt")).is_err());
    }

    #[test]
    fn write_path_allows_new_file_in_dir() {
        let dir = temp_dir("grayslate_write_new");
        let target = dir.join("brand-new.txt");
        // Does not exist yet — a normal new-file save must be allowed.
        assert!(validate_write_path(&target).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_path_allows_existing_regular_file() {
        let dir = temp_dir("grayslate_write_regular");
        let target = dir.join("existing.txt");
        std::fs::write(&target, "hi").unwrap();
        assert!(validate_write_path(&target).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_path_rejects_overwriting_a_directory() {
        let dir = temp_dir("grayslate_write_dir_target");
        // The target itself is a directory — refuse to "save" over it.
        assert!(validate_write_path(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_regular_file_accepts_file_rejects_dir() {
        let dir = temp_dir("grayslate_regular_file_check");
        let file = dir.join("f.txt");
        std::fs::write(&file, "x").unwrap();

        let file_meta = std::fs::metadata(&file).unwrap();
        assert!(ensure_regular_file(&file_meta).is_ok());

        let dir_meta = std::fs::metadata(&dir).unwrap();
        assert!(ensure_regular_file(&dir_meta).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg(unix)]
    fn notes_root_rejects_filesystem_root() {
        assert!(validate_notes_root_choice(Path::new("/")).is_err());
    }

    #[test]
    fn notes_root_rejects_a_regular_file() {
        let dir = temp_dir("grayslate_notes_root_file");
        let file = dir.join("not-a-dir.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(validate_notes_root_choice(&file).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn notes_root_accepts_a_normal_subdirectory() {
        let dir = temp_dir("grayslate_notes_root_ok");
        assert!(validate_notes_root_choice(&dir).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn last_active_file_requires_an_absolute_path() {
        assert!(validate_last_active_file_choice("relative/file.txt").is_err());
    }

    #[test]
    fn last_active_file_rejects_a_directory() {
        let dir = temp_dir("grayslate_last_active_directory");
        assert!(validate_last_active_file_choice(dir.to_str().unwrap()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn last_active_file_allows_a_missing_absolute_path() {
        let dir = temp_dir("grayslate_last_active_missing");
        let missing = dir.join("moved.txt");
        assert!(validate_last_active_file_choice(missing.to_str().unwrap()).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
