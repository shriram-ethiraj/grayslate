use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use tauri::Window;

use crate::{
    document::{classify_existing_document, DocumentRegistry, DocumentRights},
    search,
    storage::{AppStorage, FileSource},
};

const MAX_SEARCH_RESULTS_LIMIT: usize = 200;
const SEARCH_CANCELLED_MESSAGE: &str = "Search cancelled.";

#[derive(serde::Serialize)]
pub struct AuthorizedSearchResultRecord {
    #[serde(flatten)]
    result: search::types::SearchResultRecord,
    document_id: String,
    document_generation: u64,
}

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

    /// Cancel any in-flight search for the given window without starting a
    /// replacement.  Called by the frontend when the query is cleared, the
    /// sidebar is closed, or the component tears down.
    fn cancel_active(&self, window_label: &str) {
        let mut active_searches = self
            .cancellations
            .active_searches
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if let Some(active) = active_searches.remove(window_label) {
            active.cancelled.store(true, Ordering::Relaxed);
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
    documents: tauri::State<'_, DocumentRegistry>,
    search_state: tauri::State<'_, SearchRuntimeState>,
    window: Window,
    query: String,
    filter_mode: String,
    limit: Option<usize>,
    request_id: u64,
    case_sensitive: Option<bool>,
    whole_word: Option<bool>,
    use_regex: Option<bool>,
) -> Result<Vec<AuthorizedSearchResultRecord>, String> {
    let limit = clamp_search_results_limit(limit);
    let window_label = window.label().to_string();
    let cancellation_flag = search_state.begin_request(&window_label, request_id);
    let storage = storage.inner().clone();
    let search_storage = storage.clone();
    let runtime_state = search_state.inner().clone();
    let app_handle = app.clone();

    let options = search::query::SearchOptions {
        case_sensitive: case_sensitive.unwrap_or(false),
        whole_word: whole_word.unwrap_or(false),
        use_regex: use_regex.unwrap_or(false),
    };

    let result = tauri::async_runtime::spawn_blocking(move || {
        search::run_sidebar_search(
            &app_handle,
            &search_storage,
            &runtime_state,
            &query,
            &filter_mode,
            limit,
            options,
            cancellation_flag.as_ref(),
        )
    })
    .await
    .map_err(|error| format!("Failed to join sidebar search task: {}", error))?;

    search_state.finish_request(&window_label, request_id);

    let results = match result {
        Err(error) if error == SEARCH_CANCELLED_MESSAGE => return Err(error),
        other => other?,
    };

    let mut authorized = Vec::with_capacity(results.len());
    for result in results {
        let requested_source = match result.source.as_str() {
            "slates" => FileSource::Slates,
            "local" => FileSource::Local,
            _ => continue,
        };
        let (canonical, actual_source) =
            match classify_existing_document(&app, &storage, std::path::Path::new(&result.path)) {
                Ok(value) => value,
                Err(_) => continue,
            };
        if requested_source != actual_source {
            continue;
        }
        let granted = match documents.grant_existing(
            &window_label,
            &canonical,
            actual_source,
            DocumentRights::tracked(actual_source),
        ) {
            Ok(granted) => granted,
            Err(_) => continue,
        };
        authorized.push(AuthorizedSearchResultRecord {
            result,
            document_id: granted.id,
            document_generation: granted.generation,
        });
    }
    Ok(authorized)
}

/// Immediately cancel any in-flight sidebar search for this window.
/// Called by the frontend when the query is cleared, the sidebar closes,
/// or the search component tears down — without waiting for a replacement
/// search to arrive.
#[tauri::command]
pub fn cancel_sidebar_search(search_state: tauri::State<'_, SearchRuntimeState>, window: Window) {
    search_state.cancel_active(window.label());
}
