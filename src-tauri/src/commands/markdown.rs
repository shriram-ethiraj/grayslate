use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

use tauri::Window;

use crate::markdown_preview;

const MAX_MARKDOWN_PREVIEW_BYTES: usize = 5 * 1024 * 1024;
const MAX_MARKDOWN_ASSET_BYTES: u64 = 25 * 1024 * 1024;
const MARKDOWN_ASSET_EXTENSIONS: &[&str] = &[
    "avif", "bmp", "gif", "ico", "jpeg", "jpg", "png", "svg", "webp",
];

#[derive(Clone)]
struct ActiveRender {
    generation: u64,
    cancelled: Arc<AtomicBool>,
}

/// Per-window markdown preview state.  Tracks the currently in-flight
/// render so a new request automatically cancels the previous one,
/// exactly like `EditorFindState` does for search scans.
#[derive(Clone, Default)]
pub struct MarkdownPreviewState {
    active_renders: Arc<Mutex<HashMap<String, ActiveRender>>>,
    next_generation: Arc<AtomicU64>,
}

impl MarkdownPreviewState {
    /// Register a new render for a window, cancelling any previous
    /// in-flight render for the same window.
    fn begin_render(&self, window_label: &str) -> (u64, Arc<AtomicBool>) {
        let mut active = self
            .active_renders
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let cancelled = Arc::new(AtomicBool::new(false));
        if let Some(previous) = active.insert(
            window_label.to_string(),
            ActiveRender {
                generation,
                cancelled: Arc::clone(&cancelled),
            },
        ) {
            previous.cancelled.store(true, Ordering::Relaxed);
        }
        (generation, cancelled)
    }

    /// Remove the active-render entry if it still matches this backend-owned
    /// generation. Frontend request IDs may restart when a preview remounts.
    fn finish_render(&self, window_label: &str, generation: u64) {
        let mut active = self
            .active_renders
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let should_remove = active
            .get(window_label)
            .map(|a| a.generation == generation)
            .unwrap_or(false);
        if should_remove {
            active.remove(window_label);
        }
    }

    /// Cancel any in-flight render for this window.
    fn cancel_active(&self, window_label: &str) {
        let mut active = self
            .active_renders
            .lock()
            .unwrap_or_else(|p| p.into_inner());
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
) -> Result<tauri::ipc::Response, String> {
    if content.len() > MAX_MARKDOWN_PREVIEW_BYTES {
        state.cancel_active(window.label());
        return Err(format!(
            "Markdown preview is limited to {} MB",
            MAX_MARKDOWN_PREVIEW_BYTES / (1024 * 1024)
        ));
    }

    let window_label = window.label().to_string();
    let (generation, cancelled) = state.begin_render(&window_label);
    let state_clone = state.inner().clone();

    let joined = tauri::async_runtime::spawn_blocking(move || {
        markdown_preview::render_markdown_to_html(&content, cancelled.as_ref())
    })
    .await;

    state_clone.finish_render(&window_label, generation);
    let result = joined.map_err(|e| format!("Failed to join markdown render task: {e}"))?;

    match result {
        Ok(html) => Ok(tauri::ipc::Response::new(html.into_bytes())),
        Err(e) => Err(e),
    }
}

/// Cancel any in-flight markdown render for this window.  Called on
/// component teardown and rapid re-renders.
#[tauri::command]
pub fn cancel_markdown_preview(state: tauri::State<'_, MarkdownPreviewState>, window: Window) {
    state.cancel_active(window.label());
}

fn resolve_markdown_asset_path(
    document_path: &str,
    resource_path: &str,
) -> Result<PathBuf, String> {
    let document = std::fs::canonicalize(document_path)
        .map_err(|error| format!("Could not resolve Markdown document: {error}"))?;
    if !document.is_file() {
        return Err("Markdown document path is not a file".to_string());
    }

    let resource_without_suffix = resource_path.split(['?', '#']).next().unwrap_or_default();
    let decoded = urlencoding::decode(resource_without_suffix)
        .map_err(|_| "Markdown image path contains invalid percent encoding".to_string())?;
    let relative = Path::new(decoded.as_ref());
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err("Markdown image path must be relative to the document".to_string());
    }

    let extension = relative
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "Markdown image has no supported extension".to_string())?;
    if !MARKDOWN_ASSET_EXTENSIONS.contains(&extension.as_str()) {
        return Err("Markdown preview only loads supported image files".to_string());
    }

    let parent = document
        .parent()
        .ok_or_else(|| "Markdown document has no parent directory".to_string())?;
    let asset = std::fs::canonicalize(parent.join(relative))
        .map_err(|error| format!("Could not resolve Markdown image: {error}"))?;
    if !asset.starts_with(parent) {
        return Err("Markdown image escaped the document directory".to_string());
    }
    let link_metadata = std::fs::symlink_metadata(&asset)
        .map_err(|error| format!("Could not inspect Markdown image: {error}"))?;
    if link_metadata.file_type().is_symlink() {
        return Err("Markdown image cannot be a symlink".to_string());
    }
    if !asset.is_file() {
        return Err("Markdown image path is not a file".to_string());
    }

    Ok(asset)
}

/// Read a relative image referenced by a saved Markdown document. The command
/// accepts images only, applies a hard size cap, and returns raw bytes so the
/// frontend can use a short-lived blob URL without enabling Tauri's broad
/// filesystem asset protocol.
#[tauri::command]
pub async fn read_markdown_preview_asset(
    app: tauri::AppHandle,
    storage: tauri::State<'_, crate::storage::AppStorage>,
    documents: tauri::State<'_, crate::document::DocumentRegistry>,
    window: tauri::Window,
    document_id: String,
    document_generation: u64,
    resource_path: String,
) -> Result<tauri::ipc::Response, String> {
    let document = documents.resolve(
        window.label(),
        &document_id,
        document_generation,
        crate::document::DocumentAccess::Read,
    )?;
    crate::document::revalidate_source_authority(&app, storage.inner(), &document)?;
    let document_path = document.path.to_string_lossy().into_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let asset = resolve_markdown_asset_path(&document_path, &resource_path)?;
        let metadata = std::fs::metadata(&asset)
            .map_err(|error| format!("Could not inspect Markdown image: {error}"))?;
        if metadata.len() > MAX_MARKDOWN_ASSET_BYTES {
            return Err(format!(
                "Markdown images are limited to {} MB",
                MAX_MARKDOWN_ASSET_BYTES / (1024 * 1024)
            ));
        }

        let mut file = crate::document::open_authorized_read(&asset)?;
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.by_ref()
            .take(MAX_MARKDOWN_ASSET_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Could not read Markdown image: {error}"))?;
        if bytes.len() as u64 > MAX_MARKDOWN_ASSET_BYTES {
            return Err("Markdown image exceeded the size limit while reading".to_string());
        }
        Ok(tauri::ipc::Response::new(bytes))
    })
    .await
    .map_err(|error| format!("Failed to join Markdown image task: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_markdown_asset_extensions_are_lowercase() {
        assert!(MARKDOWN_ASSET_EXTENSIONS
            .iter()
            .all(|extension| *extension == extension.to_ascii_lowercase()));
    }

    #[test]
    fn resolves_images_relative_to_the_markdown_document() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let document = repository_root.join("README.md");

        let icon = resolve_markdown_asset_path(document.to_string_lossy().as_ref(), "app-icon.png")
            .unwrap();
        let hero =
            resolve_markdown_asset_path(document.to_string_lossy().as_ref(), "docs/hero.png")
                .unwrap();

        assert!(icon.ends_with("app-icon.png"));
        assert!(hero.ends_with(Path::new("docs").join("hero.png")));
    }

    #[test]
    fn rejects_non_image_markdown_assets() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let document = repository_root.join("README.md");
        let result = resolve_markdown_asset_path(document.to_string_lossy().as_ref(), "README.md");

        assert!(result.is_err());
    }

    #[test]
    fn rejects_markdown_image_traversal_outside_document_directory() {
        let root =
            std::env::temp_dir().join(format!("grayslate-markdown-{}", uuid::Uuid::new_v4()));
        let document_dir = root.join("docs");
        std::fs::create_dir_all(&document_dir).unwrap();
        let document = document_dir.join("note.md");
        let outside = root.join("outside.png");
        std::fs::write(&document, "![outside](../outside.png)").unwrap();
        std::fs::write(&outside, "image").unwrap();

        let result =
            resolve_markdown_asset_path(document.to_string_lossy().as_ref(), "../outside.png");
        assert!(result.is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
