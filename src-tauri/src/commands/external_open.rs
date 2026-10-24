use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use serde::Serialize;
use tauri::{Emitter, Manager};
use url::Url;

use crate::{
    document::{
        classify_existing_document, AuthorizedDocument, DocumentDescriptor, DocumentRegistry,
        DocumentRights,
    },
    storage::AppStorage,
    window::WindowRegistry,
};

use super::{file::MAX_FILE_SIZE, RECENT_FILES_UPDATED_EVENT};

pub const MAX_FILES_PER_OPEN_REQUEST: usize = 50;

pub const EXTERNAL_OPEN_PENDING_EVENT: &str = "files://external-open-pending";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalOpenRequest {
    pub request_id: String,
    pub preserve_current: bool,
    pub duplicate_count: usize,
    pub limit_skipped_count: usize,
    pub documents: Vec<DocumentDescriptor>,
    pub requested_count: usize,
    pub accepted_count: usize,
    pub newly_tracked_count: usize,
    pub skipped_count: usize,
    pub routing_failure_count: usize,
}

#[derive(Default)]
pub struct ExternalOpenState {
    pending: Mutex<HashMap<String, VecDeque<ExternalOpenRequest>>>,
    in_flight: Mutex<HashMap<String, ExternalOpenRequest>>,
    dropped: Mutex<DroppedPathQueue>,
    prepared: tokio::sync::Notify,
}

struct DroppedPathBatch {
    preserve_current: bool,
    window_label: String,
    paths: Vec<PathBuf>,
}

#[derive(Default)]
struct DroppedPathQueue {
    worker_running: bool,
    batches: VecDeque<DroppedPathBatch>,
}

#[derive(Default)]
struct StagedPathActivationQueue {
    pending: Mutex<VecDeque<Vec<PathBuf>>>,
}

static STAGED_PATH_ACTIVATIONS: OnceLock<StagedPathActivationQueue> = OnceLock::new();

impl StagedPathActivationQueue {
    fn push(&self, paths: Vec<PathBuf>) {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push_back(paths);
    }

    fn drain_if_ready(&self, ready: bool) -> Vec<Vec<PathBuf>> {
        if !ready {
            return Vec::new();
        }

        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .drain(..)
            .collect()
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty()
    }
}

impl ExternalOpenState {
    fn push(&self, window_label: &str, request: ExternalOpenRequest) {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entry(window_label.to_string())
            .or_default()
            .push_back(request);
    }

    fn pop(&self, window_label: &str) -> Option<ExternalOpenRequest> {
        let mut all_pending = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let pending = all_pending.get_mut(window_label)?;
        let request = pending.pop_front();
        if pending.is_empty() {
            all_pending.remove(window_label);
        }
        if let Some(request) = &request {
            self.in_flight
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(window_label.to_string(), request.clone());
        }
        request
    }

    fn complete(&self, window_label: &str, request_id: &str) {
        let mut in_flight = self.in_flight.lock().unwrap_or_else(|p| p.into_inner());
        if in_flight
            .get(window_label)
            .is_some_and(|request| request.request_id == request_id)
        {
            in_flight.remove(window_label);
        }
    }

    pub fn cleanup_window(&self, window_label: &str) -> Vec<Vec<PathBuf>> {
        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        let mut requests = Vec::new();
        if let Some(request) = self
            .in_flight
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(window_label)
        {
            requests.push(request);
        }
        requests.extend(pending.remove(window_label).unwrap_or_default());
        requests
            .into_iter()
            .filter(|request| request.preserve_current)
            .map(|request| {
                request
                    .documents
                    .into_iter()
                    .map(|document| PathBuf::from(document.display_path))
                    .collect()
            })
            .collect()
    }

    /// Returns true only for the caller responsible for starting the worker.
    fn push_dropped(&self, batch: DroppedPathBatch) -> bool {
        let mut dropped = self
            .dropped
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        dropped.batches.push_back(batch);
        if dropped.worker_running {
            return false;
        }
        dropped.worker_running = true;
        true
    }

    fn pop_dropped(&self) -> Option<DroppedPathBatch> {
        let mut dropped = self
            .dropped
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let batch = dropped.batches.pop_front();
        if batch.is_none() {
            dropped.worker_running = false;
            self.prepared.notify_waiters();
        }
        batch
    }

    async fn wait_for_preparation(&self) {
        loop {
            let prepared = self.prepared.notified();
            tokio::pin!(prepared);
            prepared.as_mut().enable();
            if !self
                .dropped
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .worker_running
            {
                return;
            }
            prepared.await;
        }
    }

    #[cfg(feature = "e2e")]
    pub fn dropped_paths_idle(&self) -> Result<bool, String> {
        let dropped = self
            .dropped
            .lock()
            .map_err(|_| "Dropped-path queue is poisoned.".to_string())?;
        Ok(!dropped.worker_running && dropped.batches.is_empty())
    }
}

/// Bring the most recently focused Grayslate window forward without treating
/// focus failure as a file-open failure. Window managers may deny focus stealing.
pub fn focus_main_window(app: &tauri::AppHandle) {
    crate::window::focus_preferred_window(app);
}

/// Resolve argv values delivered by the OS or the single-instance plugin.
/// Relative paths are interpreted against the launcher's working directory;
/// only local `file:` URLs are accepted.
pub fn enqueue_cli_activation(
    app: &tauri::AppHandle,
    args: impl IntoIterator<Item = String>,
    cwd: &Path,
) {
    let paths = args
        .into_iter()
        .filter_map(|argument| argument_to_path(&argument, cwd))
        .collect::<Vec<_>>();
    if activation_states_ready(app) {
        enqueue_paths_to_preferred_window(app, paths);
    } else {
        enqueue_path_activation(app, paths);
    }
}

pub fn enqueue_initial_activation(app: &tauri::AppHandle) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let paths = std::env::args_os()
        .skip(1)
        .filter_map(|argument| match argument.to_str() {
            Some(argument) => argument_to_path(argument, &cwd),
            None => {
                let path = PathBuf::from(argument);
                Some(if path.is_absolute() {
                    path
                } else {
                    cwd.join(path)
                })
            }
        })
        .collect::<Vec<_>>();
    let has_paths = !paths.is_empty();
    enqueue_path_activation(app, paths);
    if has_paths {
        focus_main_window(app);
    }
}

fn staged_path_activations() -> &'static StagedPathActivationQueue {
    STAGED_PATH_ACTIVATIONS.get_or_init(StagedPathActivationQueue::default)
}

fn activation_states_ready(app: &tauri::AppHandle) -> bool {
    app.try_state::<AppStorage>().is_some()
        && app.try_state::<DocumentRegistry>().is_some()
        && app.try_state::<ExternalOpenState>().is_some()
        && app.try_state::<WindowRegistry>().is_some()
}

fn stage_and_drain_if_ready(
    queue: &StagedPathActivationQueue,
    paths: Vec<PathBuf>,
    ready: impl FnOnce() -> bool,
) -> Vec<Vec<PathBuf>> {
    if paths.is_empty() {
        return Vec::new();
    }

    // Stage before checking readiness. If setup completes concurrently, either
    // its flush drains this batch or the readiness check below does; the batch
    // cannot land after setup's final flush with nobody left to process it.
    queue.push(paths);
    queue.drain_if_ready(ready())
}

fn enqueue_path_activation(app: &tauri::AppHandle, paths: Vec<PathBuf>) {
    let batches = stage_and_drain_if_ready(staged_path_activations(), paths, || {
        activation_states_ready(app)
    });
    for paths in batches {
        enqueue_paths_to_preferred_window(app, paths);
    }
}

/// Native OS activation can arrive before Tauri's setup callback. Keep every
/// path batch process-local until storage, document grants, and request state
/// have all been managed, then replay the batches in arrival order.
pub fn flush_staged_path_activations(app: &tauri::AppHandle) {
    let batches = staged_path_activations().drain_if_ready(activation_states_ready(app));
    for paths in batches {
        enqueue_paths_to_preferred_window(app, paths);
    }
}

pub fn enqueue_paths_to_preferred_window(app: &tauri::AppHandle, paths: Vec<PathBuf>) {
    enqueue_native_batch(
        app,
        DroppedPathBatch {
            window_label: String::new(),
            preserve_current: true,
            paths,
        },
    );
}

#[cfg(any(target_os = "macos", test))]
fn opened_urls_to_paths(urls: impl IntoIterator<Item = Url>) -> Vec<PathBuf> {
    urls.into_iter()
        .filter_map(|url| url.to_file_path().ok())
        .collect()
}

#[cfg(target_os = "macos")]
pub fn enqueue_opened_urls(app: &tauri::AppHandle, urls: Vec<Url>) {
    let paths = opened_urls_to_paths(urls);
    enqueue_path_activation(app, paths);
}

/// Accept file paths supplied by Tauri's native drag/drop window event.
///
/// This is deliberately a Rust-only boundary: exposing an IPC command that
/// accepts arbitrary paths would let untrusted webview code forge a drop and
/// grant itself filesystem access without a native user gesture.
pub fn enqueue_dropped_paths(app: &tauri::AppHandle, window_label: &str, paths: Vec<PathBuf>) {
    enqueue_native_batch(
        app,
        DroppedPathBatch {
            window_label: window_label.to_string(),
            preserve_current: false,
            paths,
        },
    );
}

fn enqueue_native_batch(app: &tauri::AppHandle, batch: DroppedPathBatch) {
    if batch.paths.is_empty() {
        return;
    }
    let state = app.state::<ExternalOpenState>();
    if !state.push_dropped(batch) {
        return;
    }
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || loop {
        let Some(batch) = worker_app.state::<ExternalOpenState>().pop_dropped() else {
            return;
        };
        let label = if batch.preserve_current {
            #[cfg(target_os = "macos")]
            if worker_app
                .state::<WindowRegistry>()
                .preferred_window_label(&worker_app)
                .is_none()
            {
                let _ = crate::window::reopen_or_create_main_window(&worker_app);
            }
            worker_app
                .state::<WindowRegistry>()
                .preferred_window_label(&worker_app)
        } else {
            worker_app
                .get_webview_window(&batch.window_label)
                .map(|_| batch.window_label.clone())
        };
        let Some(label) = label else {
            continue;
        };
        let mut request = prepare_document_batch(&worker_app, &label, batch.paths.clone());
        request.preserve_current = batch.preserve_current;
        // Do not recreate grants/queues for a window destroyed during preparation.
        // The registry lock serializes this check with Destroyed cleanup.
        let queued = worker_app
            .state::<WindowRegistry>()
            .while_window_live(&label, || {
                worker_app
                    .state::<ExternalOpenState>()
                    .push(&label, request);
            });
        if queued {
            let _ = worker_app.emit_to(&*label, EXTERNAL_OPEN_PENDING_EVENT, ());
        } else {
            worker_app.state::<DocumentRegistry>().revoke_window(&label);
            if batch.preserve_current {
                worker_app.state::<ExternalOpenState>().push_dropped(batch);
            }
        }
    });
}

fn argument_to_path(argument: &str, cwd: &Path) -> Option<PathBuf> {
    if argument.is_empty() || argument.starts_with('-') {
        return None;
    }

    if argument
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:"))
    {
        return Url::parse(argument)
            .ok()
            .and_then(|url| url.to_file_path().ok());
    }
    if argument.contains("://") {
        return None;
    }

    let path = PathBuf::from(argument);
    Some(if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    })
}

struct PreparedPaths {
    documents: Vec<AuthorizedDocument>,
    requested_count: usize,
    newly_tracked_count: usize,
    skipped_count: usize,
    duplicate_count: usize,
    limit_skipped_count: usize,
}

fn prepare_paths(app: &tauri::AppHandle, window_label: &str, paths: Vec<PathBuf>) -> PreparedPaths {
    if paths.is_empty() {
        return PreparedPaths {
            documents: Vec::new(),
            requested_count: 0,
            newly_tracked_count: 0,
            skipped_count: 0,
            duplicate_count: 0,
            limit_skipped_count: 0,
        };
    }

    let requested_count = paths.len();
    let storage = app.state::<AppStorage>();
    let documents = app.state::<DocumentRegistry>();
    let (candidates, duplicate_count, limit_skipped_count) = select_candidates(paths, |path| {
        let (canonical, source) = classify_existing_document(app, &storage, path).ok()?;
        let metadata = std::fs::metadata(&canonical).ok()?;
        (metadata.len() <= MAX_FILE_SIZE).then_some((canonical, source))
    });

    let mut accepted = Vec::new();
    let mut newly_tracked_count = 0;
    for (canonical, source) in candidates {
        match storage.record_file_open_if_untracked(&canonical, source) {
            Ok(inserted) => {
                newly_tracked_count += usize::from(inserted);
                accepted.push((canonical, source));
            }
            Err(error) => {
                eprintln!("[External Open] Failed to track an incoming file: {error}");
            }
        }
    }

    let granted = accepted
        .iter()
        .filter_map(|(path, source)| {
            documents
                .grant_existing(
                    window_label,
                    path,
                    *source,
                    DocumentRights::tracked(*source),
                )
                .map_err(|error| {
                    eprintln!("[External Open] Failed to authorize an incoming file: {error}");
                    error
                })
                .ok()
        })
        .collect::<Vec<_>>();

    let accepted_count = granted.len();
    let skipped_count =
        requested_count.saturating_sub(accepted_count + duplicate_count + limit_skipped_count);
    if newly_tracked_count > 0 {
        let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());
    }

    PreparedPaths {
        documents: granted,
        requested_count,
        newly_tracked_count,
        skipped_count,
        duplicate_count,
        limit_skipped_count,
    }
}

pub(crate) async fn prepare_document_batch_async(
    app: tauri::AppHandle,
    window_label: String,
    paths: Vec<PathBuf>,
) -> Result<ExternalOpenRequest, String> {
    tauri::async_runtime::spawn_blocking(move || prepare_document_batch(&app, &window_label, paths))
        .await
        .map_err(|error| format!("File preparation failed: {error}"))
}

pub(crate) fn prepare_document_batch(
    app: &tauri::AppHandle,
    window_label: &str,
    paths: Vec<PathBuf>,
) -> ExternalOpenRequest {
    let prepared = prepare_paths(app, window_label, paths);
    ExternalOpenRequest {
        request_id: uuid::Uuid::now_v7().to_string(),
        preserve_current: false,
        duplicate_count: prepared.duplicate_count,
        limit_skipped_count: prepared.limit_skipped_count,
        accepted_count: prepared.documents.len(),
        documents: prepared
            .documents
            .into_iter()
            .map(|document| document.descriptor())
            .collect(),
        requested_count: prepared.requested_count,
        newly_tracked_count: prepared.newly_tracked_count,
        skipped_count: prepared.skipped_count,
        routing_failure_count: 0,
    }
}

/// Deduplicate validated identities before imposing the per-request allowance.
/// Keeping the last occurrence preserves the existing OS ordering contract.
fn select_candidates<T>(
    paths: Vec<PathBuf>,
    mut validate: impl FnMut(&Path) -> Option<(PathBuf, T)>,
) -> (Vec<(PathBuf, T)>, usize, usize) {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let mut duplicates = 0;
    for path in paths.into_iter().rev() {
        let Some((canonical, value)) = validate(&path) else {
            continue;
        };
        if !seen.insert(canonical.clone()) {
            duplicates += 1;
            continue;
        }
        candidates.push((canonical, value));
    }
    candidates.reverse();
    let overflow = candidates.len().saturating_sub(MAX_FILES_PER_OPEN_REQUEST);
    candidates.truncate(MAX_FILES_PER_OPEN_REQUEST);
    (candidates, duplicates, overflow)
}

#[tauri::command]
pub fn complete_external_open_request(
    window: tauri::Window,
    state: tauri::State<'_, ExternalOpenState>,
    request_id: String,
) {
    state.complete(window.label(), &request_id);
}

#[tauri::command]
pub async fn take_external_open_request(
    window: tauri::Window,
    state: tauri::State<'_, ExternalOpenState>,
) -> Result<Option<ExternalOpenRequest>, String> {
    // A cold launch must observe native input before considering last-file restoration.
    state.wait_for_preparation().await;
    Ok(state.pop(window.label()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        path: Option<&str>,
        requested_count: usize,
        accepted_count: usize,
        newly_tracked_count: usize,
        skipped_count: usize,
    ) -> ExternalOpenRequest {
        ExternalOpenRequest {
            request_id: uuid::Uuid::now_v7().to_string(),
            preserve_current: false,
            duplicate_count: 0,
            limit_skipped_count: 0,
            documents: path
                .into_iter()
                .map(|display_path| DocumentDescriptor {
                    document_id: display_path.to_string(),
                    generation: 1,
                    display_path: display_path.to_string(),
                    file_name: display_path.to_string(),
                    source: "local".to_string(),
                    writable: true,
                })
                .collect(),
            requested_count,
            accepted_count,
            newly_tracked_count,
            skipped_count,
            routing_failure_count: 0,
        }
    }

    #[test]
    fn file_allowance_is_fifty_per_request_not_a_process_window_limit() {
        for count in [49usize, 50, 51, 100, 50] {
            let paths = (0..count)
                .map(|i| PathBuf::from(format!("file-{i}")))
                .collect();
            let (accepted, duplicates, overflow) =
                select_candidates(paths, |path| Some((path.to_path_buf(), ())));
            assert_eq!(accepted.len(), count.min(50));
            assert_eq!(duplicates, 0);
            assert_eq!(overflow, count.saturating_sub(50));
            assert_eq!(accepted.first().unwrap().0, PathBuf::from("file-0"));
            assert_eq!(
                accepted.last().unwrap().0,
                PathBuf::from(format!("file-{}", count.min(50) - 1))
            );
        }
    }

    #[test]
    fn duplicates_and_invalid_paths_do_not_consume_the_allowance() {
        let mut paths = vec![PathBuf::from("file-0"), PathBuf::from("missing")];
        paths.extend((0..51).map(|i| PathBuf::from(format!("file-{i}"))));
        let (accepted, duplicates, overflow) = select_candidates(paths, |path| {
            (path != Path::new("missing")).then(|| (path.to_path_buf(), ()))
        });
        assert_eq!(accepted.len(), 50);
        assert_eq!(duplicates, 1);
        assert_eq!(overflow, 1);
        let (accepted, duplicates, _) = select_candidates(
            ["a", "b", "a", "c", "b"].map(PathBuf::from).to_vec(),
            |path| Some((path.to_path_buf(), ())),
        );
        assert_eq!(
            accepted
                .iter()
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>(),
            ["a", "c", "b"].map(PathBuf::from)
        );
        assert_eq!(duplicates, 2);
    }

    #[test]
    fn closing_receiver_replays_only_unacknowledged_os_requests() {
        let state = ExternalOpenState::default();
        let mut os = request(Some("os.txt"), 1, 1, 1, 0);
        os.preserve_current = true;
        state.push("main", os.clone());
        state.pop("main");
        state.push("main", request(Some("drop.txt"), 1, 1, 1, 0));
        assert_eq!(
            state.cleanup_window("main"),
            vec![vec![PathBuf::from("os.txt")]]
        );
        state.push("main", os.clone());
        state.pop("main");
        state.complete("main", &os.request_id);
        assert!(state.cleanup_window("main").is_empty());
    }

    #[test]
    fn argv_paths_resolve_against_the_launch_directory() {
        assert_eq!(
            argument_to_path("notes/example.json", Path::new("/work")),
            Some(PathBuf::from("/work/notes/example.json"))
        );
    }

    #[test]
    fn argv_accepts_only_local_file_urls() {
        let local_path = if cfg!(windows) {
            PathBuf::from(r"C:\tmp\example.json")
        } else {
            PathBuf::from("/tmp/example.json")
        };
        let local_url = Url::from_file_path(&local_path).expect("test path should form a file URL");
        assert_eq!(
            argument_to_path(local_url.as_str(), Path::new("/work")),
            Some(local_path)
        );
        assert_eq!(
            argument_to_path("https://example.com/example.json", Path::new("/work")),
            None
        );
    }

    #[test]
    fn argv_does_not_treat_windows_drive_letters_as_url_schemes() {
        assert!(argument_to_path(r"C:\Users\person\example.json", Path::new("/work")).is_some());
    }

    #[test]
    fn argv_ignores_flags() {
        assert_eq!(argument_to_path("--verbose", Path::new("/work")), None);
    }

    #[test]
    fn staged_activations_wait_for_readiness_and_drain_once() {
        let queue = StagedPathActivationQueue::default();
        let first = vec![PathBuf::from("first.json")];

        assert!(stage_and_drain_if_ready(&queue, first.clone(), || false).is_empty());
        assert!(!queue.is_empty());
        assert_eq!(queue.drain_if_ready(true), vec![first]);
        assert!(queue.drain_if_ready(true).is_empty());
        assert!(queue.is_empty());
    }

    #[test]
    fn staged_activations_are_fifo_and_ready_batches_drain_immediately() {
        let queue = StagedPathActivationQueue::default();
        let first = vec![PathBuf::from("first.json")];
        let second = vec![PathBuf::from("second.jsonl")];
        let ready = vec![PathBuf::from("ready.txt")];

        assert!(stage_and_drain_if_ready(&queue, first.clone(), || false).is_empty());
        assert!(stage_and_drain_if_ready(&queue, second.clone(), || false).is_empty());
        assert_eq!(
            stage_and_drain_if_ready(&queue, ready.clone(), || true),
            vec![first, second, ready]
        );
        assert!(queue.is_empty());
    }

    #[test]
    fn empty_and_non_file_url_activations_are_ignored() {
        let queue = StagedPathActivationQueue::default();
        assert!(stage_and_drain_if_ready(&queue, Vec::new(), || true).is_empty());
        assert!(queue.is_empty());

        let urls = vec![
            Url::parse("https://example.com/example.json").unwrap(),
            Url::parse("mailto:test@example.com").unwrap(),
        ];
        assert!(opened_urls_to_paths(urls).is_empty());
        assert!(queue.is_empty());
    }

    #[test]
    fn pending_requests_remain_fifo() {
        let state = ExternalOpenState::default();
        state.push("main", request(Some("first.txt"), 2, 1, 1, 1));
        state.push("main", request(None, 1, 0, 0, 1));
        state.push("main", request(Some("last.txt"), 3, 3, 2, 0));

        let first = state
            .pop("main")
            .expect("first request should remain queued");
        assert_eq!(first.requested_count, 2);
        assert_eq!(
            first
                .documents
                .first()
                .map(|document| document.display_path.as_str()),
            Some("first.txt")
        );
        assert_eq!(
            state.pop("main").map(|request| request.requested_count),
            Some(1)
        );
        assert_eq!(
            state.pop("main").map(|request| request.requested_count),
            Some(3)
        );
    }

    #[test]
    fn pending_requests_are_isolated_by_window() {
        let state = ExternalOpenState::default();
        state.push("main", request(Some("main.txt"), 1, 1, 0, 0));
        state.push("editor-1", request(Some("other.txt"), 1, 1, 0, 0));

        assert_eq!(
            state
                .pop("editor-1")
                .and_then(|request| request.documents.into_iter().next())
                .map(|document| document.display_path),
            Some("other.txt".to_string())
        );
        assert_eq!(
            state
                .pop("main")
                .and_then(|request| request.documents.into_iter().next())
                .map(|document| document.display_path),
            Some("main.txt".to_string())
        );
    }

    #[test]
    fn dropped_batches_are_fifo_and_only_start_one_worker() {
        let state = ExternalOpenState::default();
        assert!(state.push_dropped(DroppedPathBatch {
            preserve_current: false,
            window_label: "first".to_string(),
            paths: vec![PathBuf::from("first.txt")],
        }));
        assert!(!state.push_dropped(DroppedPathBatch {
            preserve_current: false,
            window_label: "second".to_string(),
            paths: vec![PathBuf::from("second.txt")],
        }));

        assert_eq!(
            state.pop_dropped().map(|batch| batch.window_label),
            Some("first".to_string())
        );
        assert_eq!(
            state.pop_dropped().map(|batch| batch.window_label),
            Some("second".to_string())
        );
        assert!(state.pop_dropped().is_none());

        assert!(state.push_dropped(DroppedPathBatch {
            preserve_current: false,
            window_label: "third".to_string(),
            paths: vec![PathBuf::from("third.txt")],
        }));
    }
}
