use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use serde::Serialize;
use tauri::Window;

use crate::csv::{
    self, CsvMutationRequest, CsvMutationResponse, CsvRowWindow, CsvSession, CsvTableSnapshot,
};

// ---------------------------------------------------------------------------
// Session registry — one CSV session per window
// ---------------------------------------------------------------------------

/// Per-window CSV table session state. Only one session per window at a time.
/// Uses Arc wrappers so the struct is cheaply Clone-able for moving into
/// spawn_blocking closures.
#[derive(Clone, Default)]
pub struct CsvSessionRegistry {
    sessions: Arc<Mutex<HashMap<String, CsvSession>>>,
    /// Cancellation flag for long-running init/flush operations.
    cancel_flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl CsvSessionRegistry {
    fn with_session<F, R>(&self, window_label: &str, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut CsvSession) -> R,
    {
        let mut sessions = self.sessions.lock().unwrap_or_else(|p| p.into_inner());
        let session = sessions
            .get_mut(window_label)
            .ok_or_else(|| "No active CSV session for this window.".to_string())?;
        Ok(f(session))
    }

    /// Autosave helper: flush the CSV session's serialized text and version
    /// without going through the normal command flow. Returns `None` if no
    /// session exists for this window.
    pub fn try_flush_for_autosave(&self, window_label: &str) -> Option<(u64, String)> {
        let mut sessions = self.sessions.lock().unwrap_or_else(|p| p.into_inner());
        sessions.get_mut(window_label).map(|session| {
            let version = session.version;
            let text = session.flush_text();
            (version, text)
        })
    }

    fn insert(&self, window_label: &str, session: CsvSession) {
        let mut sessions = self.sessions.lock().unwrap_or_else(|p| p.into_inner());
        sessions.insert(window_label.to_string(), session);
    }

    fn remove(&self, window_label: &str) {
        let mut sessions = self.sessions.lock().unwrap_or_else(|p| p.into_inner());
        sessions.remove(window_label);
    }

    fn begin_cancellable(&self, window_label: &str) -> Arc<AtomicBool> {
        let mut flags = self.cancel_flags.lock().unwrap_or_else(|p| p.into_inner());
        // Cancel any previous in-flight operation for this window.
        if let Some(old) = flags.get(window_label) {
            old.store(true, Ordering::Relaxed);
        }
        let flag = Arc::new(AtomicBool::new(false));
        flags.insert(window_label.to_string(), Arc::clone(&flag));
        flag
    }

    fn finish_cancellable(&self, window_label: &str) {
        let mut flags = self.cancel_flags.lock().unwrap_or_else(|p| p.into_inner());
        flags.remove(window_label);
    }

    fn cancel(&self, window_label: &str) {
        let flags = self.cancel_flags.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(flag) = flags.get(window_label) {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

// ---------------------------------------------------------------------------
// Channel events for csv_initialize progress
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CsvChannelEvent {
    /// Incremental parsing progress. `parsed_rows` is the count so far.
    #[serde(rename_all = "camelCase")]
    Progress {
        parsed_rows: usize,
    },
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvFlushResponse {
    pub version: u64,
    pub byte_length: usize,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Initialize a CSV table session: parse the text, store the session, report
/// progress via a channel. Runs on a blocking thread.
#[tauri::command]
pub async fn csv_initialize(
    text: String,
    registry: tauri::State<'_, CsvSessionRegistry>,
    window: Window,
    on_event: tauri::ipc::Channel<CsvChannelEvent>,
) -> Result<CsvTableSnapshot, String> {
    let window_label = window.label().to_string();
    let cancelled = registry.begin_cancellable(&window_label);
    let registry_clone = registry.inner().clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        csv::parse_csv(&text, &cancelled, |parsed_rows| {
            let _ = on_event.send(CsvChannelEvent::Progress { parsed_rows });
        })
    })
    .await
    .map_err(|e| format!("Failed to join CSV parse task: {}", e))?;

    registry.finish_cancellable(&window_label);

    match result {
        Ok(session) => {
            let snapshot = session.snapshot();
            registry_clone.insert(&window_label, session);
            Ok(snapshot)
        }
        Err(e) => Err(e),
    }
}

/// Dispose the CSV table session for this window, freeing all memory.
#[tauri::command]
pub fn csv_dispose(
    registry: tauri::State<'_, CsvSessionRegistry>,
    window: Window,
) {
    let window_label = window.label();
    registry.cancel(window_label);
    registry.remove(window_label);
}

/// Return a window of rows for viewport rendering.
#[tauri::command]
pub fn csv_get_rows(
    start: usize,
    end: usize,
    registry: tauri::State<'_, CsvSessionRegistry>,
    window: Window,
) -> Result<CsvRowWindow, String> {
    registry.with_session(window.label(), |session| session.get_rows(start, end))
}

/// Return a single cell value. `row_index` of -1 means header row.
#[tauri::command]
pub fn csv_get_cell(
    row_index: i64,
    col_index: usize,
    registry: tauri::State<'_, CsvSessionRegistry>,
    window: Window,
) -> Result<String, String> {
    registry.with_session(window.label(), |session| {
        session.get_cell(row_index, col_index)
    })
}

/// Apply a mutation (edit-cell, add-row, delete-columns, etc.).
#[tauri::command]
pub fn csv_mutate(
    mutation: CsvMutationRequest,
    user_event: String,
    registry: tauri::State<'_, CsvSessionRegistry>,
    window: Window,
) -> Result<CsvMutationResponse, String> {
    registry.with_session(window.label(), |session| {
        session.mutate(&mutation, &user_event)
    })
}

/// Undo the last mutation.
#[tauri::command]
pub fn csv_undo(
    registry: tauri::State<'_, CsvSessionRegistry>,
    window: Window,
) -> Result<CsvMutationResponse, String> {
    registry.with_session(window.label(), |session| session.undo())
}

/// Redo the last undone mutation.
#[tauri::command]
pub fn csv_redo(
    registry: tauri::State<'_, CsvSessionRegistry>,
    window: Window,
) -> Result<CsvMutationResponse, String> {
    registry.with_session(window.label(), |session| session.redo())
}

/// Serialize the current table state to CSV text and return as raw bytes.
/// Uses `spawn_blocking` since serialization of large tables can be expensive.
#[tauri::command]
pub async fn csv_flush_text(
    registry: tauri::State<'_, CsvSessionRegistry>,
    window: Window,
) -> Result<tauri::ipc::Response, String> {
    let window_label = window.label().to_string();
    let registry_clone = registry.inner().clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        registry_clone.with_session(&window_label, |session| {
            let text = session.flush_text();
            let version = session.flush_version();
            (text, version)
        })
    })
    .await
    .map_err(|e| format!("Failed to join CSV flush task: {}", e))??;

    let (text, _version) = result;
    Ok(tauri::ipc::Response::new(text.into_bytes()))
}

/// Cancel any in-flight long-running CSV operation (init or flush).
#[tauri::command]
pub fn csv_cancel(
    registry: tauri::State<'_, CsvSessionRegistry>,
    window: Window,
) {
    registry.cancel(window.label());
}
