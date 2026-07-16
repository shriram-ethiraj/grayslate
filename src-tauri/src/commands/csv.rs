use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
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

/// One in-flight cancellable operation for a window, tagged with a monotonic
/// generation so a stale `finish_cancellable` cannot drop a newer operation's
/// flag.
struct CsvCancelEntry {
    generation: u64,
    flag: Arc<AtomicBool>,
}

/// Per-window CSV table session state. Only one session per window at a time.
/// Uses Arc wrappers so the struct is cheaply Clone-able for moving into
/// spawn_blocking closures.
#[derive(Clone, Default)]
pub struct CsvSessionRegistry {
    sessions: Arc<Mutex<HashMap<String, CsvSession>>>,
    /// Cancellation flag for the current long-running init/flush operation
    /// per window, tagged with a generation (see `CsvCancelEntry`).
    cancel_flags: Arc<Mutex<HashMap<String, CsvCancelEntry>>>,
    /// Monotonic source of operation generations, shared across windows.
    next_generation: Arc<AtomicU64>,
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

    /// Begin a cancellable operation for `window_label`. Cancels any previous
    /// in-flight operation for the same window and returns the new operation's
    /// generation plus its fresh cancellation flag.
    fn begin_cancellable(&self, window_label: &str) -> (u64, Arc<AtomicBool>) {
        let mut flags = self.cancel_flags.lock().unwrap_or_else(|p| p.into_inner());
        // Cancel any previous in-flight operation for this window.
        if let Some(old) = flags.get(window_label) {
            old.flag.store(true, Ordering::Relaxed);
        }
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let flag = Arc::new(AtomicBool::new(false));
        flags.insert(
            window_label.to_string(),
            CsvCancelEntry {
                generation,
                flag: Arc::clone(&flag),
            },
        );
        (generation, flag)
    }

    /// Finish the operation identified by `generation`. Only removes the flag
    /// if it still belongs to that operation — a newer `begin_cancellable` may
    /// have replaced it, and removing the newer flag would make the newer
    /// operation uncancellable.
    fn finish_cancellable(&self, window_label: &str, generation: u64) {
        let mut flags = self.cancel_flags.lock().unwrap_or_else(|p| p.into_inner());
        let owns_current = flags
            .get(window_label)
            .map(|entry| entry.generation == generation)
            .unwrap_or(false);
        if owns_current {
            flags.remove(window_label);
        }
    }

    fn cancel(&self, window_label: &str) {
        let flags = self.cancel_flags.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(entry) = flags.get(window_label) {
            entry.flag.store(true, Ordering::Relaxed);
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
    Progress { parsed_rows: usize },
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
    let (generation, cancelled) = registry.begin_cancellable(&window_label);
    let registry_clone = registry.inner().clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        csv::parse_csv(&text, &cancelled, |parsed_rows| {
            let _ = on_event.send(CsvChannelEvent::Progress { parsed_rows });
        })
    })
    .await
    .map_err(|e| format!("Failed to join CSV parse task: {}", e))?;

    registry.finish_cancellable(&window_label, generation);

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
pub fn csv_dispose(registry: tauri::State<'_, CsvSessionRegistry>, window: Window) {
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
pub fn csv_cancel(registry: tauri::State<'_, CsvSessionRegistry>, window: Window) {
    registry.cancel(window.label());
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starting_a_new_op_cancels_the_previous_one() {
        let registry = CsvSessionRegistry::default();
        let (_gen_a, flag_a) = registry.begin_cancellable("main");
        assert!(!flag_a.load(Ordering::Relaxed));

        let (_gen_b, _flag_b) = registry.begin_cancellable("main");
        // Beginning B must cancel A.
        assert!(flag_a.load(Ordering::Relaxed));
    }

    #[test]
    fn stale_finish_does_not_drop_a_newer_ops_flag() {
        let registry = CsvSessionRegistry::default();
        let (gen_a, _flag_a) = registry.begin_cancellable("main");
        let (gen_b, flag_b) = registry.begin_cancellable("main");
        assert_ne!(gen_a, gen_b);

        // A finishes late — it must NOT remove B's still-active flag.
        registry.finish_cancellable("main", gen_a);

        // cancel() must still reach B (regression: previously the stale finish
        // removed B's flag and this cancel became a silent no-op).
        registry.cancel("main");
        assert!(flag_b.load(Ordering::Relaxed));
    }

    #[test]
    fn owning_finish_removes_the_flag() {
        let registry = CsvSessionRegistry::default();
        let (generation, flag) = registry.begin_cancellable("main");

        registry.finish_cancellable("main", generation);

        // After the owning op finishes, cancel is a no-op (flag was removed).
        registry.cancel("main");
        assert!(!flag.load(Ordering::Relaxed));
    }

    #[test]
    fn generations_are_unique_across_windows() {
        let registry = CsvSessionRegistry::default();
        let (gen_a, _) = registry.begin_cancellable("win-a");
        let (gen_b, _) = registry.begin_cancellable("win-b");
        assert_ne!(gen_a, gen_b);
    }
}
