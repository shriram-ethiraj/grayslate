use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Window};
use tauri_plugin_clipboard_manager::ClipboardExt;

const COPY_ID_HEADER: &str = "x-grayslate-copy-id";
const COPY_INDEX_HEADER: &str = "x-grayslate-copy-index";
const COPY_FINAL_HEADER: &str = "x-grayslate-copy-final";
const COPY_CANCEL_HEADER: &str = "x-grayslate-copy-cancel";

/// Keep individual WebView IPC messages comfortably below practical platform
/// limits. The frontend currently sends chunks no larger than 4 MiB of UTF-8.
const MAX_COPY_CHUNK_BYTES: usize = 8 * 1024 * 1024;
/// Files opened by Grayslate are capped at 200 MiB, but CSV quoting and edits
/// can expand their serialized representation. Keep a bounded ceiling while
/// allowing that worst-case growth.
const MAX_CLIPBOARD_COPY_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug)]
struct PendingClipboardCopy {
    request_id: String,
    next_chunk_index: usize,
    bytes: Vec<u8>,
}

#[derive(Clone, Default)]
pub struct ClipboardCopyRegistry {
    pending: Arc<Mutex<HashMap<String, PendingClipboardCopy>>>,
}

impl ClipboardCopyRegistry {
    fn append(
        &self,
        window_label: &str,
        request_id: &str,
        chunk_index: usize,
        chunk: Vec<u8>,
        final_chunk: bool,
    ) -> Result<Option<Vec<u8>>, String> {
        if chunk.len() > MAX_COPY_CHUNK_BYTES {
            return Err(format!(
                "Clipboard copy chunk exceeds the {} MiB limit.",
                MAX_COPY_CHUNK_BYTES / (1024 * 1024)
            ));
        }

        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());

        if chunk_index == 0 {
            pending.insert(
                window_label.to_string(),
                PendingClipboardCopy {
                    request_id: request_id.to_string(),
                    next_chunk_index: 0,
                    bytes: Vec::with_capacity(chunk.len()),
                },
            );
        }

        let transfer = pending
            .get_mut(window_label)
            .ok_or_else(|| "No active clipboard copy for this window.".to_string())?;

        if transfer.request_id != request_id {
            return Err("Clipboard copy was replaced by a newer request.".to_string());
        }
        if transfer.next_chunk_index != chunk_index {
            return Err(format!(
                "Expected clipboard chunk {}, received {}.",
                transfer.next_chunk_index, chunk_index
            ));
        }
        if transfer.bytes.len().saturating_add(chunk.len()) > MAX_CLIPBOARD_COPY_BYTES {
            pending.remove(window_label);
            return Err(format!(
                "Clipboard copy exceeds the {} MiB limit.",
                MAX_CLIPBOARD_COPY_BYTES / (1024 * 1024)
            ));
        }

        transfer.bytes.extend_from_slice(&chunk);
        transfer.next_chunk_index += 1;

        if !final_chunk {
            return Ok(None);
        }

        Ok(pending.remove(window_label).map(|transfer| transfer.bytes))
    }

    fn cancel(&self, window_label: &str, request_id: &str) {
        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        let owns_transfer = pending
            .get(window_label)
            .map(|transfer| transfer.request_id == request_id)
            .unwrap_or(false);
        if owns_transfer {
            pending.remove(window_label);
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardCopyResponse {
    pub completed: bool,
    pub byte_length: usize,
}

fn required_header(request: &Request<'_>, name: &str) -> Result<String, String> {
    request
        .headers()
        .get(name)
        .ok_or_else(|| format!("Missing clipboard request header '{name}'."))?
        .to_str()
        .map(str::to_string)
        .map_err(|_| format!("Invalid clipboard request header '{name}'."))
}

fn enabled_header(request: &Request<'_>, name: &str) -> bool {
    request
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        == Some("1")
}

pub(crate) fn write_text(app: &AppHandle, text: String) -> Result<usize, String> {
    let byte_length = text.len();
    app.clipboard()
        .write_text(text)
        .map_err(|error| format!("Failed to write to clipboard: {error}"))?;
    Ok(byte_length)
}

/// Receive one raw UTF-8 chunk from the editor. The registry assembles chunks
/// per window, then the final request validates UTF-8 and writes to the native
/// clipboard on a blocking worker thread.
#[tauri::command]
pub async fn clipboard_write_chunk(
    request: Request<'_>,
    registry: tauri::State<'_, ClipboardCopyRegistry>,
    window: Window,
    app: AppHandle,
) -> Result<ClipboardCopyResponse, String> {
    let request_id = required_header(&request, COPY_ID_HEADER)?;
    if request_id.len() > 128 {
        return Err("Clipboard request identifier is too long.".to_string());
    }

    let window_label = window.label().to_string();
    let registry = registry.inner().clone();

    if enabled_header(&request, COPY_CANCEL_HEADER) {
        registry.cancel(&window_label, &request_id);
        return Ok(ClipboardCopyResponse {
            completed: false,
            byte_length: 0,
        });
    }

    let chunk_index = required_header(&request, COPY_INDEX_HEADER)?
        .parse::<usize>()
        .map_err(|_| "Invalid clipboard chunk index.".to_string())?;
    let final_chunk = enabled_header(&request, COPY_FINAL_HEADER);
    let chunk = match request.body() {
        InvokeBody::Raw(bytes) => bytes.clone(),
        InvokeBody::Json(_) => {
            return Err("Clipboard chunks must use the raw-byte IPC transport.".to_string())
        }
    };

    let completed = registry.append(&window_label, &request_id, chunk_index, chunk, final_chunk)?;

    let Some(bytes) = completed else {
        return Ok(ClipboardCopyResponse {
            completed: false,
            byte_length: 0,
        });
    };

    tauri::async_runtime::spawn_blocking(move || {
        let text = String::from_utf8(bytes)
            .map_err(|_| "Clipboard copy contained invalid UTF-8 text.".to_string())?;
        let byte_length = write_text(&app, text)?;
        Ok(ClipboardCopyResponse {
            completed: true,
            byte_length,
        })
    })
    .await
    .map_err(|error| format!("Failed to join clipboard copy task: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_chunks_in_order_and_removes_completed_transfer() {
        let registry = ClipboardCopyRegistry::default();
        assert!(registry
            .append("main", "copy-a", 0, b"hello ".to_vec(), false)
            .unwrap()
            .is_none());

        let bytes = registry
            .append("main", "copy-a", 1, b"world".to_vec(), true)
            .unwrap()
            .unwrap();
        assert_eq!(bytes, b"hello world");
        assert!(registry
            .append("main", "copy-a", 2, Vec::new(), true)
            .is_err());
    }

    #[test]
    fn rejects_out_of_order_chunks() {
        let registry = ClipboardCopyRegistry::default();
        registry
            .append("main", "copy-a", 0, b"first".to_vec(), false)
            .unwrap();

        let error = registry
            .append("main", "copy-a", 2, b"third".to_vec(), true)
            .unwrap_err();
        assert!(error.contains("Expected clipboard chunk 1"));
    }

    #[test]
    fn cancellation_only_removes_the_matching_request() {
        let registry = ClipboardCopyRegistry::default();
        registry
            .append("main", "copy-a", 0, b"first".to_vec(), false)
            .unwrap();

        registry.cancel("main", "copy-b");
        assert!(registry
            .append("main", "copy-a", 1, b"second".to_vec(), true)
            .unwrap()
            .is_some());
    }

    #[test]
    fn a_new_first_chunk_replaces_the_previous_request() {
        let registry = ClipboardCopyRegistry::default();
        registry
            .append("main", "copy-a", 0, b"old".to_vec(), false)
            .unwrap();
        registry
            .append("main", "copy-b", 0, b"new".to_vec(), false)
            .unwrap();

        let error = registry
            .append("main", "copy-a", 1, b"stale".to_vec(), true)
            .unwrap_err();
        assert!(error.contains("newer request"));
    }
}
