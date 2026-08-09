use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use serde::Serialize;
use tauri::{Emitter, Manager};
use url::Url;

use crate::{
    document::{classify_existing_document, DocumentDescriptor, DocumentRegistry, DocumentRights},
    storage::AppStorage,
};

use super::{file::MAX_FILE_SIZE, RECENT_FILES_UPDATED_EVENT};

pub const EXTERNAL_OPEN_PENDING_EVENT: &str = "files://external-open-pending";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalOpenRequest {
    pub document: Option<DocumentDescriptor>,
    pub requested_count: usize,
    pub accepted_count: usize,
    pub newly_tracked_count: usize,
    pub skipped_count: usize,
}

#[derive(Default)]
pub struct ExternalOpenState {
    pending: Mutex<VecDeque<ExternalOpenRequest>>,
    dropped: Mutex<DroppedPathQueue>,
}

struct DroppedPathBatch {
    window_label: String,
    paths: Vec<PathBuf>,
}

#[derive(Default)]
struct DroppedPathQueue {
    worker_running: bool,
    batches: VecDeque<DroppedPathBatch>,
}

static EARLY_CLI_ACTIVATIONS: OnceLock<Mutex<VecDeque<Vec<PathBuf>>>> = OnceLock::new();

impl ExternalOpenState {
    fn push(&self, request: ExternalOpenRequest) {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push_back(request);
    }

    fn pop(&self) -> Option<ExternalOpenRequest> {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut merged = pending.pop_front()?;
        for request in pending.drain(..) {
            if request.document.is_some() {
                merged.document = request.document;
            }
            merged.requested_count += request.requested_count;
            merged.accepted_count += request.accepted_count;
            merged.newly_tracked_count += request.newly_tracked_count;
            merged.skipped_count += request.skipped_count;
        }
        Some(merged)
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
        }
        batch
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

/// Bring the single Grayslate window forward without treating focus failure as
/// a file-open failure. Window managers are allowed to deny focus stealing.
pub fn focus_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
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
    stage_cli_paths(paths);
    flush_staged_cli_activations(app);
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
    stage_cli_paths(paths);
    flush_staged_cli_activations(app);
    if has_paths {
        focus_main_window(app);
    }
}

fn stage_cli_paths(paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
    EARLY_CLI_ACTIVATIONS
        .get_or_init(|| Mutex::new(VecDeque::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push_back(paths);
}

/// A secondary instance can reach the plugin during the narrow interval
/// between plugin setup and application setup. Keep those argv batches in a
/// process-local staging queue until the storage and document states exist.
pub fn flush_staged_cli_activations(app: &tauri::AppHandle) {
    if app.try_state::<AppStorage>().is_none()
        || app.try_state::<DocumentRegistry>().is_none()
        || app.try_state::<ExternalOpenState>().is_none()
    {
        return;
    }

    let Some(staged) = EARLY_CLI_ACTIVATIONS.get() else {
        return;
    };
    let batches = {
        let mut staged = staged
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        staged.drain(..).collect::<Vec<_>>()
    };
    for paths in batches {
        enqueue_paths(app, "main", paths);
    }
}

#[cfg(target_os = "macos")]
pub fn enqueue_opened_urls(app: &tauri::AppHandle, urls: Vec<Url>) {
    let paths = urls
        .into_iter()
        .filter_map(|url| url.to_file_path().ok())
        .collect::<Vec<_>>();
    enqueue_paths(app, "main", paths);
}

/// Accept file paths supplied by Tauri's native drag/drop window event.
///
/// This is deliberately a Rust-only boundary: exposing an IPC command that
/// accepts arbitrary paths would let untrusted webview code forge a drop and
/// grant itself filesystem access without a native user gesture.
pub fn enqueue_dropped_paths(app: &tauri::AppHandle, window_label: &str, paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }

    let state = app.state::<ExternalOpenState>();
    let should_start_worker = state.push_dropped(DroppedPathBatch {
        window_label: window_label.to_string(),
        paths,
    });
    if !should_start_worker {
        return;
    }

    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || loop {
        let Some(batch) = worker_app.state::<ExternalOpenState>().pop_dropped() else {
            return;
        };
        enqueue_paths(&worker_app, &batch.window_label, batch.paths);
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

fn enqueue_paths(app: &tauri::AppHandle, window_label: &str, paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }

    let requested_count = paths.len();
    let storage = app.state::<AppStorage>();
    let documents = app.state::<DocumentRegistry>();
    let mut candidates = Vec::new();
    let mut accepted_paths = HashSet::new();

    // Iterate from the end so duplicate paths keep their final OS ordering.
    // Reverse once more before tracking so recency also follows OS ordering.
    for path in paths.into_iter().rev() {
        let Ok((canonical, source)) = classify_existing_document(app, &storage, &path) else {
            continue;
        };
        if !accepted_paths.insert(canonical.clone()) {
            continue;
        }
        let Ok(metadata) = std::fs::metadata(&canonical) else {
            continue;
        };
        if metadata.len() > MAX_FILE_SIZE {
            continue;
        }
        candidates.push((canonical, source));
    }
    candidates.reverse();

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

    let document = accepted.last().and_then(|(path, source)| {
        documents
            .grant_existing(
                window_label,
                path,
                *source,
                DocumentRights::tracked(*source),
            )
            .map(|document| document.descriptor())
            .map_err(|error| {
                eprintln!("[External Open] Failed to authorize an incoming file: {error}");
                error
            })
            .ok()
    });

    let accepted_count = accepted.len();
    let skipped_count = requested_count.saturating_sub(accepted_count);
    if newly_tracked_count > 0 {
        let _ = app.emit(RECENT_FILES_UPDATED_EVENT, ());
    }

    app.state::<ExternalOpenState>().push(ExternalOpenRequest {
        document,
        requested_count,
        accepted_count,
        newly_tracked_count,
        skipped_count,
    });
    let _ = app.emit(EXTERNAL_OPEN_PENDING_EVENT, ());
}

#[tauri::command]
pub fn take_external_open_request(
    state: tauri::State<'_, ExternalOpenState>,
) -> Option<ExternalOpenRequest> {
    state.pop()
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
            document: path.map(|display_path| DocumentDescriptor {
                document_id: display_path.to_string(),
                generation: 1,
                display_path: display_path.to_string(),
                file_name: display_path.to_string(),
                source: "local".to_string(),
                writable: true,
            }),
            requested_count,
            accepted_count,
            newly_tracked_count,
            skipped_count,
        }
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
    fn pending_requests_merge_counts_and_keep_the_last_document() {
        let state = ExternalOpenState::default();
        state.push(request(Some("first.txt"), 2, 1, 1, 1));
        state.push(request(None, 1, 0, 0, 1));
        state.push(request(Some("last.txt"), 3, 3, 2, 0));

        let merged = state.pop().expect("queued requests should merge");
        assert_eq!(merged.requested_count, 6);
        assert_eq!(merged.accepted_count, 4);
        assert_eq!(merged.newly_tracked_count, 3);
        assert_eq!(merged.skipped_count, 2);
        assert_eq!(
            merged.document.map(|document| document.display_path),
            Some("last.txt".to_string())
        );
        assert!(state.pop().is_none());
    }

    #[test]
    fn dropped_batches_are_fifo_and_only_start_one_worker() {
        let state = ExternalOpenState::default();
        assert!(state.push_dropped(DroppedPathBatch {
            window_label: "first".to_string(),
            paths: vec![PathBuf::from("first.txt")],
        }));
        assert!(!state.push_dropped(DroppedPathBatch {
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
            window_label: "third".to_string(),
            paths: vec![PathBuf::from("third.txt")],
        }));
    }
}
