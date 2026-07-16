use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use serde::Serialize;
use tauri::Window;

use crate::findstats::{self, FindStatsOptions, ScanCache};

#[derive(Clone)]
struct ActiveScan {
    request_id: u64,
    cancelled: Arc<AtomicBool>,
}

/// Per-window editor find state, separate from the sidebar search runtime.
/// Uses Arc wrappers so the struct is cheaply Clone-able for moving into
/// spawn_blocking closures.
#[derive(Clone, Default)]
pub struct EditorFindState {
    active_scans: Arc<Mutex<HashMap<String, ActiveScan>>>,
    caches: Arc<Mutex<HashMap<String, ScanCache>>>,
}

impl EditorFindState {
    /// Register a new scan for a window, cancelling any previous in-flight scan.
    fn begin_scan(&self, window_label: &str, request_id: u64) -> Arc<AtomicBool> {
        let mut active = self.active_scans.lock().unwrap_or_else(|p| p.into_inner());
        let cancelled = Arc::new(AtomicBool::new(false));
        if let Some(previous) = active.insert(
            window_label.to_string(),
            ActiveScan {
                request_id,
                cancelled: Arc::clone(&cancelled),
            },
        ) {
            previous.cancelled.store(true, Ordering::Relaxed);
        }
        cancelled
    }

    /// Store scan results in the cache and remove the active-scan entry.
    fn finish_scan(&self, window_label: &str, request_id: u64, cache: ScanCache) {
        let mut active = self.active_scans.lock().unwrap_or_else(|p| p.into_inner());
        let should_remove = active
            .get(window_label)
            .map(|a| a.request_id == request_id)
            .unwrap_or(false);
        if should_remove {
            active.remove(window_label);
        }

        self.caches
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(window_label.to_string(), cache);
    }

    /// Cancel any in-flight scan and clear the cache for this window.
    fn cancel_active(&self, window_label: &str) {
        let mut active = self.active_scans.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(scan) = active.remove(window_label) {
            scan.cancelled.store(true, Ordering::Relaxed);
        }
        self.caches
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(window_label);
    }

    fn with_cache<F, R>(&self, window_label: &str, f: F) -> Option<R>
    where
        F: FnOnce(&ScanCache) -> R,
    {
        let caches = self.caches.lock().unwrap_or_else(|p| p.into_inner());
        caches.get(window_label).map(f)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorFindResponse {
    pub request_id: u64,
    pub match_count: usize,
    pub current_match: usize,
    pub approximate: bool,
}

/// Full-document scan: receives text + query + options, runs on a blocking
/// thread, caches results, and returns match stats.
#[tauri::command]
pub async fn editor_find_scan(
    state: tauri::State<'_, EditorFindState>,
    window: Window,
    text: String,
    search: String,
    case_sensitive: bool,
    whole_word: bool,
    use_regex: bool,
    selection_from: usize,
    selection_to: usize,
    request_id: u64,
) -> Result<EditorFindResponse, String> {
    let window_label = window.label().to_string();
    let cancellation_flag = state.begin_scan(&window_label, request_id);
    let state_clone = state.inner().clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let options = FindStatsOptions {
            case_sensitive,
            whole_word,
            use_regex,
        };

        findstats::scan(
            &text,
            &search,
            &options,
            selection_from,
            selection_to,
            cancellation_flag.as_ref(),
        )
    })
    .await
    .map_err(|e| format!("Failed to join find scan task: {}", e))?;

    match result {
        Ok((stats, cache)) => {
            state_clone.finish_scan(&window_label, request_id, cache);
            Ok(EditorFindResponse {
                request_id,
                match_count: stats.match_count,
                current_match: stats.current_match,
                approximate: stats.approximate,
            })
        }
        Err(e) => Err(e),
    }
}

/// Selection-only update: reuses cached match positions to recompute currentMatch
/// without rescanning the document.
#[tauri::command]
pub fn editor_find_selection(
    state: tauri::State<'_, EditorFindState>,
    window: Window,
    selection_from: usize,
    selection_to: usize,
    request_id: u64,
) -> EditorFindResponse {
    let window_label = window.label();
    state
        .with_cache(window_label, |cache| {
            let result = findstats::current_match_from_cache(cache, selection_from, selection_to);
            EditorFindResponse {
                request_id,
                match_count: result.match_count,
                current_match: result.current_match,
                approximate: result.approximate,
            }
        })
        .unwrap_or(EditorFindResponse {
            request_id,
            match_count: 0,
            current_match: 0,
            approximate: false,
        })
}

/// Cancel any in-flight editor find scan and clear the cache for this window.
#[tauri::command]
pub fn cancel_editor_find(state: tauri::State<'_, EditorFindState>, window: Window) {
    state.cancel_active(window.label());
}
