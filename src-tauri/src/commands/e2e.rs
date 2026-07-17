//! End-to-end test-only IPC commands.
//!
//! Compiled ONLY under `--features e2e` (see the `e2e` feature in `Cargo.toml`)
//! and never registered in a distributed release build. Each command runs the
//! exact production authorization + grant path used by the real open / save-as
//! handlers, substituting a caller-provided fixture path for the native file
//! dialog that WebDriver cannot drive. No new file authority is introduced:
//! grants still flow through `classify_*` + `DocumentRegistry`, so an e2e test
//! exercises the same code a real user's dialog pick would.

use std::path::PathBuf;

use tauri::Emitter;

use crate::document::{
    classify_existing_document, classify_new_document, DocumentDescriptor, DocumentRegistry,
    DocumentRights,
};
use crate::storage::AppStorage;

/// Frontend open event (mirror of `OPEN_FILE_PATH_EVENT` in `recentFiles.ts`).
/// The sidebar emits this in production after a grant; the open shim emits it
/// too so a single test call drives the real `openAuthorizedDocument` flow.
const OPEN_FILE_PATH_EVENT: &str = "files://open-path";

/// Matches the frontend `OpenFilePathPayload` shape (camelCase keys).
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OpenFilePathPayload {
    document_id: String,
    document_generation: u64,
    path: String,
    source: String,
}

/// Open a fixture file exactly as `pick_document` does after the user chooses a
/// file in the native open dialog: classify it, grant a tracked authorization
/// for the current window, and return the descriptor. The frontend then drives
/// its normal open flow with the returned grant.
#[tauri::command]
pub async fn e2e_open_path(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    path: String,
) -> Result<Option<DocumentDescriptor>, String> {
    let path = PathBuf::from(path);
    let (canonical, source) = classify_existing_document(&app, storage.inner(), &path)?;
    let granted = documents.grant_existing(
        window.label(),
        &canonical,
        source,
        DocumentRights::tracked(source),
    )?;
    let descriptor = granted.descriptor();

    // Emit the same open event the sidebar emits so the frontend loads the file
    // through its real authorized-open handler.
    window
        .emit(
            OPEN_FILE_PATH_EVENT,
            OpenFilePathPayload {
                document_id: descriptor.document_id.clone(),
                document_generation: descriptor.generation,
                path: descriptor.display_path.clone(),
                source: descriptor.source.clone(),
            },
        )
        .map_err(|error| format!("Failed to emit open event: {error}"))?;

    Ok(Some(descriptor))
}

/// Grant a Save-As target exactly as `pick_save_document` does after the user
/// chooses a path in the native save dialog: existing paths are re-classified,
/// new paths are validated as new documents, then a tracked grant is issued.
#[tauri::command]
pub async fn e2e_save_path(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    path: String,
) -> Result<Option<DocumentDescriptor>, String> {
    let path = PathBuf::from(path);

    let (authorized_path, source, exists) = if path.exists() {
        let (canonical, source) = classify_existing_document(&app, storage.inner(), &path)?;
        (canonical, source, true)
    } else {
        let (candidate, source) = classify_new_document(&app, storage.inner(), &path)?;
        (candidate, source, false)
    };

    let granted = if exists {
        documents.grant_existing(
            window.label(),
            &authorized_path,
            source,
            DocumentRights::tracked(source),
        )?
    } else {
        documents.grant_new(
            window.label(),
            &authorized_path,
            source,
            DocumentRights::tracked(source),
        )?
    };
    Ok(Some(granted.descriptor()))
}
