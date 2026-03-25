use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use tauri::Window;

use crate::markdown_preview;

#[derive(Clone)]
struct ActiveRender {
    request_id: u64,
    cancelled: Arc<AtomicBool>,
}

/// Per-window markdown preview state.  Tracks the currently in-flight
/// render so a new request automatically cancels the previous one,
/// exactly like `EditorFindState` does for search scans.
#[derive(Clone, Default)]
pub struct MarkdownPreviewState {
    active_renders: Arc<Mutex<HashMap<String, ActiveRender>>>,
}

impl MarkdownPreviewState {
    /// Register a new render for a window, cancelling any previous
    /// in-flight render for the same window.
    fn begin_render(&self, window_label: &str, request_id: u64) -> Arc<AtomicBool> {
        let mut active = self.active_renders.lock().unwrap_or_else(|p| p.into_inner());
        let cancelled = Arc::new(AtomicBool::new(false));
        if let Some(previous) = active.insert(
            window_label.to_string(),
            ActiveRender {
                request_id,
                cancelled: Arc::clone(&cancelled),
            },
        ) {
            previous.cancelled.store(true, Ordering::Relaxed);
        }
        cancelled
    }

    /// Remove the active-render entry if it still matches the given
    /// request_id (a newer request may have already replaced it).
    fn finish_render(&self, window_label: &str, request_id: u64) {
        let mut active = self.active_renders.lock().unwrap_or_else(|p| p.into_inner());
        let should_remove = active
            .get(window_label)
            .map(|a| a.request_id == request_id)
            .unwrap_or(false);
        if should_remove {
            active.remove(window_label);
        }
    }

    /// Cancel any in-flight render for this window.
    fn cancel_active(&self, window_label: &str) {
        let mut active = self.active_renders.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(render) = active.remove(window_label) {
            render.cancelled.store(true, Ordering::Relaxed);
        }
    }
}

/// Render markdown source to sanitized preview HTML.  Runs the heavy
/// render work on a blocking thread so the Tauri async runtime stays
/// responsive.  Returns raw UTF-8 bytes consumed by `invokeText()` on
/// the frontend.
#[tauri::command]
pub async fn render_markdown_preview(
    state: tauri::State<'_, MarkdownPreviewState>,
    window: Window,
    content: String,
    request_id: u64,
) -> Result<tauri::ipc::Response, String> {
    let window_label = window.label().to_string();
    let cancelled = state.begin_render(&window_label, request_id);
    let state_clone = state.inner().clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        markdown_preview::render_markdown_to_html(&content, cancelled.as_ref())
    })
    .await
    .map_err(|e| format!("Failed to join markdown render task: {}", e))?;

    state_clone.finish_render(&window_label, request_id);

    match result {
        Ok(html) => Ok(tauri::ipc::Response::new(html.into_bytes())),
        Err(e) => Err(e),
    }
}

/// Cancel any in-flight markdown render for this window.  Called on
/// component teardown and rapid re-renders.
#[tauri::command]
pub fn cancel_markdown_preview(
    state: tauri::State<'_, MarkdownPreviewState>,
    window: Window,
) {
    state.cancel_active(window.label());
}
