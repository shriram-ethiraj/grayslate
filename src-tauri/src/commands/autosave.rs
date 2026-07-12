/**
 * commands/autosave.rs
 *
 * Tauri commands that form the frontend-facing API of the backend-driven
 * autosave engine.  See `autosave.rs` for the core scheduling logic.
 *
 * Commands:
 *   - `autosave_register`        — register a document for autosave tracking
 *   - `autosave_notify_changed`  — lightweight "content changed" signal (no content)
 *   - `autosave_submit_content`  — FE responds to a content request with serialized text
 *   - `autosave_flush_before_switch` — FE sends content before switching files
 *   - `autosave_set_csv_mode`    — toggle CSV table mode awareness
 *   - `classify_source`          — expose `classify_file_source()` to the frontend
 */

use std::path::PathBuf;

use tauri::Emitter;

use crate::autosave::{
    autosave_write_to_disk, AutosaveRegistry, DocumentCreatedPayload,
    AUTOSAVE_DOCUMENT_CREATED_EVENT,
};
use crate::commands::csv::CsvSessionRegistry;
use crate::filesystem::classify_file_source;
use crate::storage::{AppStorage, FileSource};

use super::{naming::save_new_slate_to_disk, RECENT_FILES_UPDATED_EVENT};

// ---------------------------------------------------------------------------
// autosave_register
// ---------------------------------------------------------------------------

/// Register a document for autosave tracking when it is opened or created.
/// Call this after the editor loads the document content.
///
/// - `path`: absolute path on disk, or empty string for an untitled document.
/// - `source`: `"slates"` or `"local"`.
/// - `language_hint`: current language mode (passed to naming pipeline for untitled slates).
#[tauri::command]
pub fn autosave_register(
    window: tauri::Window,
    registry: tauri::State<'_, AutosaveRegistry>,
    path: String,
    source: String,
    language_hint: String,
) {
    let file_source = match source.as_str() {
        "slates" => FileSource::Slates,
        _ => FileSource::Local,
    };
    let path_buf = if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(&path))
    };
    registry.register(window.label(), path_buf, file_source, language_hint);
}

// ---------------------------------------------------------------------------
// autosave_notify_changed
// ---------------------------------------------------------------------------

/// Lightweight change notification.  The frontend calls this whenever the
/// editor content changes (piggybacked on VALUE_SYNC).  The payload is
/// just a monotonic generation counter — no content crosses the IPC boundary.
#[tauri::command]
pub fn autosave_notify_changed(
    window: tauri::Window,
    registry: tauri::State<'_, AutosaveRegistry>,
    generation: u64,
) {
    registry.notify_changed(window.label(), generation);
}

// ---------------------------------------------------------------------------
// autosave_submit_content
// ---------------------------------------------------------------------------

/// Frontend responds to an `autosave://request-content` event by calling this
/// command with the serialized editor content and current generation.
///
/// For untitled slates (no path yet), runs the naming pipeline to create the
/// file and emits `autosave://document-created` back to the frontend.
#[tauri::command]
pub async fn autosave_submit_content(
    app: tauri::AppHandle,
    window: tauri::Window,
    registry: tauri::State<'_, AutosaveRegistry>,
    storage: tauri::State<'_, AppStorage>,
    request_id: u64,
    generation: u64,
    content: String,
) -> Result<(), String> {
    let window_label = window.label().to_string();

    // Validate request_id to reject stale submissions
    if !registry.validate_request(&window_label, request_id) {
        return Ok(()); // Silently ignore stale submissions
    }

    let doc_info = registry
        .get_document_info(&window_label)
        .ok_or_else(|| "No autosave document registered for this window.".to_string())?;

    match doc_info.path {
        Some(path) => {
            // Existing file — write using atomic temp+rename
            let path_clone = path.clone();
            let content_clone = content.clone();
            tauri::async_runtime::spawn_blocking(move || {
                autosave_write_to_disk(&path_clone, &content_clone)
            })
            .await
            .map_err(|e| format!("Autosave: join error: {}", e))??;

            storage.record_file_update(&path, FileSource::Slates)?;
            let _ = app.emit(RECENT_FILES_UPDATED_EVENT, "saved");
            registry.complete_save(&window_label, generation);
        }
        None => {
            // Untitled slate with no content yet (e.g. typed then deleted
            // everything before the first save) — nothing worth naming or
            // writing to disk. Mark the save complete so the timer stops
            // retrying; the file gets created once real content arrives.
            if content.is_empty() {
                registry.complete_save(&window_label, generation);
                return Ok(());
            }

            // Untitled slate — run naming pipeline to create the file
            let result = save_new_slate_to_disk(
                &app,
                storage.inner(),
                &content,
                &doc_info.language_hint,
            )
            .await?;

            let new_path = PathBuf::from(&result.path);
            registry.update_path(&window_label, new_path);
            registry.complete_save(&window_label, generation);

            // Notify the frontend of the new path so it can update
            // activeDocument and the title bar.
            let _ = window.emit(
                AUTOSAVE_DOCUMENT_CREATED_EVENT,
                DocumentCreatedPayload {
                    path: result.path,
                    detected_language: result.detected_language,
                },
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// autosave_flush_before_switch
// ---------------------------------------------------------------------------

/// Called by the frontend just before switching to a different file.
/// The content is provided inline so Rust can write immediately without
/// a roundtrip.  The command returns quickly; the actual write happens
/// on a blocking thread.
///
/// For CSV table mode, serializes directly from CsvSession instead of
/// using the provided `content` parameter.
#[tauri::command]
pub async fn autosave_flush_before_switch(
    app: tauri::AppHandle,
    window: tauri::Window,
    registry: tauri::State<'_, AutosaveRegistry>,
    storage: tauri::State<'_, AppStorage>,
    csv_registry: tauri::State<'_, CsvSessionRegistry>,
    content: String,
    generation: u64,
) -> Result<(), String> {
    let window_label = window.label().to_string();

    let doc_info = match registry.get_document_info(&window_label) {
        Some(info) => info,
        None => return Ok(()), // No document tracked
    };

    // Only flush slate files
    if !matches!(doc_info.source, FileSource::Slates) || !doc_info.is_dirty {
        registry.unregister(&window_label);
        return Ok(());
    }

    // Determine the content to save
    let (save_content, save_generation) = if doc_info.csv_table_active {
        // Serialize directly from CsvSession
        match csv_registry.try_flush_for_autosave(&window_label) {
            Some((version, text)) => (text, version),
            None => (content.clone(), generation),
        }
    } else {
        (content.clone(), generation)
    };

    match doc_info.path {
        Some(path) => {
            let path_clone = path.clone();
            tauri::async_runtime::spawn_blocking(move || {
                autosave_write_to_disk(&path_clone, &save_content)
            })
            .await
            .map_err(|e| format!("Autosave flush: join error: {}", e))??;

            storage.record_file_update(&path, FileSource::Slates)?;
            let _ = app.emit(RECENT_FILES_UPDATED_EVENT, "saved");
            registry.complete_save(&window_label, save_generation);
        }
        None => {
            // Untitled slate with no content — nothing worth naming or
            // writing to disk, just drop it on switch.
            if !save_content.is_empty() {
                // Untitled slate — create the file via naming pipeline
                let result = save_new_slate_to_disk(
                    &app,
                    storage.inner(),
                    &save_content,
                    &doc_info.language_hint,
                )
                .await?;

                let new_path = PathBuf::from(&result.path);
                registry.update_path(&window_label, new_path);
                registry.complete_save(&window_label, save_generation);

                // Emit document-created so FE can update its state
                // (though the FE is about to switch documents, it may
                // still need the path for sidebar consistency).
                let _ = window.emit(
                    AUTOSAVE_DOCUMENT_CREATED_EVENT,
                    DocumentCreatedPayload {
                        path: result.path,
                        detected_language: result.detected_language,
                    },
                );
            }
        }
    }

    // Unregister — the next file will re-register
    registry.unregister(&window_label);
    Ok(())
}

// ---------------------------------------------------------------------------
// autosave_set_csv_mode
// ---------------------------------------------------------------------------

/// Toggle CSV table mode awareness. When active, the autosave timer
/// serializes directly from CsvSession instead of requesting content
/// from the frontend.
#[tauri::command]
pub fn autosave_set_csv_mode(
    window: tauri::Window,
    registry: tauri::State<'_, AutosaveRegistry>,
    active: bool,
) {
    registry.set_csv_mode(window.label(), active);
}

// ---------------------------------------------------------------------------
// classify_source
// ---------------------------------------------------------------------------

/// Expose `classify_file_source()` to the frontend.  Returns `"slates"` or
/// `"local"` for a given absolute path.
#[tauri::command]
pub fn classify_source(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    path: String,
) -> Result<String, String> {
    let path_buf = PathBuf::from(&path);
    let source = classify_file_source(&app, storage.inner(), &path_buf)?;
    Ok(source.as_str().to_string())
}
