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
        enqueue_paths(app, paths);
    }
}

#[cfg(target_os = "macos")]
pub fn enqueue_opened_urls(app: &tauri::AppHandle, urls: Vec<Url>) {
    let paths = urls
        .into_iter()
        .filter_map(|url| url.to_file_path().ok())
        .collect::<Vec<_>>();
    enqueue_paths(app, paths);
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

fn enqueue_paths(app: &tauri::AppHandle, paths: Vec<PathBuf>) {
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
            .grant_existing("main", path, *source, DocumentRights::tracked(*source))
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

    #[test]
    fn argv_paths_resolve_against_the_launch_directory() {
        assert_eq!(
            argument_to_path("notes/example.json", Path::new("/work")),
            Some(PathBuf::from("/work/notes/example.json"))
        );
    }

    #[test]
    fn argv_accepts_only_local_file_urls() {
        assert_eq!(
            argument_to_path("file:///tmp/example.json", Path::new("/work")),
            Some(PathBuf::from("/tmp/example.json"))
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
}
