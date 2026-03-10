use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use tauri::Window;

use crate::{search, storage::AppStorage};

const MAX_SEARCH_RESULTS_LIMIT: usize = 200;
const SEARCH_CANCELLED_MESSAGE: &str = "Search cancelled.";

#[derive(Clone)]
struct ActiveSearch {
    request_id: u64,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
struct SearchCancellationRegistry {
    active_searches: Mutex<HashMap<String, ActiveSearch>>,
}

#[derive(Default)]
struct SearchStatsCache {
    average_document_length: Mutex<Option<f32>>,
}

#[derive(Clone, Default)]
pub struct SearchRuntimeState {
    cancellations: Arc<SearchCancellationRegistry>,
    stats: Arc<SearchStatsCache>,
}

impl SearchRuntimeState {
    fn begin_request(&self, window_label: &str, request_id: u64) -> Arc<AtomicBool> {
        let mut active_searches = self
            .cancellations
            .active_searches
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let cancelled = Arc::new(AtomicBool::new(false));

        if let Some(previous) = active_searches.insert(
            window_label.to_string(),
            ActiveSearch {
                request_id,
                cancelled: Arc::clone(&cancelled),
            },
        ) {
            previous.cancelled.store(true, Ordering::Relaxed);
        }

        cancelled
    }

    fn finish_request(&self, window_label: &str, request_id: u64) {
        let mut active_searches = self
            .cancellations
            .active_searches
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let should_remove = active_searches
            .get(window_label)
            .map(|active| active.request_id == request_id)
            .unwrap_or(false);

        if should_remove {
            active_searches.remove(window_label);
        }
    }

    pub fn average_document_length(&self) -> Option<f32> {
        *self
            .stats
            .average_document_length
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn update_average_document_length(&self, value: Option<f32>) {
        let mut average_document_length = self
            .stats
            .average_document_length
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *average_document_length = value;
    }
}

fn clamp_search_results_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(80).clamp(1, MAX_SEARCH_RESULTS_LIMIT)
}

#[tauri::command]
pub async fn search_sidebar_files(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    search_state: tauri::State<'_, SearchRuntimeState>,
    window: Window,
    query: String,
    filter_mode: String,
    limit: Option<usize>,
    request_id: u64,
) -> Result<Vec<search::types::SearchResultRecord>, String> {
    let limit = clamp_search_results_limit(limit);
    let window_label = window.label().to_string();
    let cancellation_flag = search_state.begin_request(&window_label, request_id);
    let storage = storage.inner().clone();
    let runtime_state = search_state.inner().clone();
    let app_handle = app.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        search::run_sidebar_search(
            &app_handle,
            &storage,
            &runtime_state,
            &query,
            &filter_mode,
            limit,
            cancellation_flag.as_ref(),
        )
    })
    .await
    .map_err(|error| format!("Failed to join sidebar search task: {}", error))?;

    search_state.finish_request(&window_label, request_id);

    match result {
        Err(error) if error == SEARCH_CANCELLED_MESSAGE => Err(error),
        other => other,
    }
}