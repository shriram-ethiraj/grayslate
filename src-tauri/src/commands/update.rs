use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Mutex,
    },
    time::Duration,
};

#[cfg(not(feature = "e2e"))]
use std::{collections::HashMap, path::PathBuf, sync::atomic::AtomicU64};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
#[cfg(not(feature = "e2e"))]
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::Notify;

#[cfg(not(feature = "e2e"))]
use crate::commands::autosave::flush_before_exit;
use crate::{
    autosave::AutosaveRegistry,
    commands::csv::CsvSessionRegistry,
    document::DocumentRegistry,
    save_coordinator::SaveCoordinator,
    storage::{AppStorage, SETTING_AUTOMATIC_UPDATE_CHECKS},
    update_policy::{current_update_policy, UpdatePolicy},
};

#[cfg(not(feature = "e2e"))]
use crate::storage::FileSource;

const UPDATE_IDLE: u8 = 0;
const UPDATE_CHECKING: u8 = 1;
const UPDATE_INSTALLING: u8 = 2;
const AUTOMATIC_UPDATE_STARTUP_DELAY: Duration = Duration::from_secs(5);
const AUTOMATIC_UPDATE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
#[cfg(not(feature = "e2e"))]
const INSTALL_APPROVAL_TIMEOUT: Duration = Duration::from_secs(5 * 60);
#[cfg(not(feature = "e2e"))]
const INSTALL_APPROVAL_RETRY_INTERVAL: Duration = Duration::from_millis(500);
pub const UPDATE_STATUS_EVENT: &str = "updates://status";
pub const UPDATE_INSTALL_PREFLIGHT_EVENT: &str = "updates://install-preflight";
pub const UPDATE_INSTALL_PREFLIGHT_RELEASE_EVENT: &str = "updates://install-preflight-release";

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum UpdateCheckResponse {
    UpToDate {
        message: String,
        current_version: String,
    },
    Available {
        message: String,
        current_version: String,
        version: String,
        published_at: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdateInstallResponse {
    pub version: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateCommandError {
    code: &'static str,
    message: String,
}

impl UpdateCommandError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Default)]
pub struct UpdateOperationState {
    operation: AtomicU8,
    status: Mutex<UpdateStatusSnapshot>,
    #[cfg(not(feature = "e2e"))]
    next_install_approval_id: AtomicU64,
    pending_install_approval: Mutex<Option<PendingInstallApproval>>,
}

#[derive(Debug)]
struct PendingInstallApproval {
    request_id: u64,
    window_label: String,
    allowed: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg(not(feature = "e2e"))]
struct UpdateInstallPreflightPayload {
    request_id: u64,
    restore_update_dialog: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(not(feature = "e2e"))]
struct ApprovedDocumentSnapshot {
    source: FileSource,
    path: Option<PathBuf>,
    document_id: Option<String>,
    document_generation: Option<u64>,
    generation: u64,
    is_dirty: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum UpdateStatusSnapshot {
    #[default]
    Idle,
    Checking {
        source: &'static str,
    },
    UpToDate {
        source: &'static str,
        message: String,
        current_version: String,
    },
    Available {
        source: &'static str,
        message: String,
        current_version: String,
        version: String,
        published_at: Option<String>,
    },
    Installing {
        message: String,
    },
    Installed {
        version: String,
        message: String,
    },
    Error {
        source: &'static str,
        message: String,
    },
}

#[derive(Default)]
pub struct AutomaticUpdateScheduler {
    wake: Notify,
}

impl UpdateOperationState {
    fn begin(&self, operation: u8) -> Result<UpdateOperationGuard<'_>, UpdateCommandError> {
        self.operation
            .compare_exchange(UPDATE_IDLE, operation, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                UpdateCommandError::new("busy", "Another update operation is already in progress.")
            })?;

        Ok(UpdateOperationGuard { state: self })
    }

    fn snapshot(&self) -> UpdateStatusSnapshot {
        self.status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn publish(&self, app: &AppHandle, status: UpdateStatusSnapshot) {
        *self
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = status.clone();
        let _ = app.emit(UPDATE_STATUS_EVENT, status);
    }

    fn blocks_automatic_check(&self) -> bool {
        matches!(
            self.snapshot(),
            UpdateStatusSnapshot::Available { .. }
                | UpdateStatusSnapshot::Installing { .. }
                | UpdateStatusSnapshot::Installed { .. }
        )
    }

    pub(crate) fn is_installing(&self) -> bool {
        self.operation.load(Ordering::Acquire) == UPDATE_INSTALLING
    }

    #[cfg(not(feature = "e2e"))]
    fn begin_install_approval(&self, window_label: &str) -> Result<u64, UpdateCommandError> {
        let request_id = self
            .next_install_approval_id
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        let mut pending = self
            .pending_install_approval
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if pending.is_some() {
            return Err(UpdateCommandError::new(
                "preflight-busy",
                "Another update-install confirmation is already pending.",
            ));
        }
        *pending = Some(PendingInstallApproval {
            request_id,
            window_label: window_label.to_string(),
            allowed: None,
        });
        Ok(request_id)
    }

    fn respond_to_install_approval(
        &self,
        window_label: &str,
        request_id: u64,
        allowed: bool,
    ) -> Result<(), String> {
        let mut pending = self
            .pending_install_approval
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(approval) = pending.as_mut() else {
            return Err("No update-install confirmation is pending.".to_string());
        };
        if approval.request_id != request_id || approval.window_label != window_label {
            return Err("The update-install confirmation is invalid or expired.".to_string());
        }
        approval.allowed = Some(allowed);
        Ok(())
    }

    #[cfg(not(feature = "e2e"))]
    fn take_install_approval_response(&self, window_label: &str, request_id: u64) -> Option<bool> {
        let mut pending = self
            .pending_install_approval
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let response = pending.as_ref().and_then(|approval| {
            (approval.request_id == request_id && approval.window_label == window_label)
                .then_some(approval.allowed)
                .flatten()
        });
        if response.is_some() {
            *pending = None;
        }
        response
    }

    #[cfg(not(feature = "e2e"))]
    fn clear_install_approval(&self, window_label: &str, request_id: u64) {
        let mut pending = self
            .pending_install_approval
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if pending.as_ref().is_some_and(|approval| {
            approval.request_id == request_id && approval.window_label == window_label
        }) {
            *pending = None;
        }
    }

    fn clear_pending_install_approval(&self) {
        *self
            .pending_install_approval
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }

    fn clear_automatic_status(&self, app: &AppHandle) {
        let is_automatic = match self.snapshot() {
            UpdateStatusSnapshot::Checking { source }
            | UpdateStatusSnapshot::UpToDate { source, .. }
            | UpdateStatusSnapshot::Available { source, .. }
            | UpdateStatusSnapshot::Error { source, .. } => source == "automatic",
            _ => false,
        };
        if is_automatic {
            self.publish(app, UpdateStatusSnapshot::Idle);
        }
    }
}

#[cfg(not(feature = "e2e"))]
fn document_snapshot(
    registry: &AutosaveRegistry,
    window_label: &str,
) -> Option<ApprovedDocumentSnapshot> {
    registry
        .get_document_info(window_label)
        .map(|document| ApprovedDocumentSnapshot {
            source: document.source,
            path: document.path,
            document_id: document.document_id,
            document_generation: document.document_generation,
            generation: document.generation,
            is_dirty: document.is_dirty,
        })
}

#[cfg(not(feature = "e2e"))]
fn validate_approved_local_documents(
    app: &AppHandle,
    registry: &AutosaveRegistry,
    approved: &HashMap<String, Option<ApprovedDocumentSnapshot>>,
) -> Result<(), UpdateCommandError> {
    for window_label in app.webview_windows().keys() {
        let Some(approved_snapshot) = approved.get(window_label) else {
            return Err(UpdateCommandError::new(
                "window-changed",
                "A new editor window opened while the update was being prepared. Installation was cancelled.",
            ));
        };
        let current = document_snapshot(registry, window_label);
        if current.as_ref().is_some_and(|document| {
            document.source == FileSource::Local
                && document.is_dirty
                && current.as_ref() != approved_snapshot.as_ref()
        }) {
            return Err(UpdateCommandError::new(
                "document-changed",
                "A local document changed after update installation was approved. Installation was cancelled to protect those edits.",
            ));
        }
    }
    Ok(())
}

impl AutomaticUpdateScheduler {
    pub fn settings_changed(&self) {
        self.wake.notify_one();
    }
}

struct UpdateOperationGuard<'a> {
    state: &'a UpdateOperationState,
}

struct UpdateInstallPreflightGuard<'a> {
    app: AppHandle,
    state: &'a UpdateOperationState,
}

impl Drop for UpdateOperationGuard<'_> {
    fn drop(&mut self) {
        self.state.operation.store(UPDATE_IDLE, Ordering::Release);
    }
}

impl Drop for UpdateInstallPreflightGuard<'_> {
    fn drop(&mut self) {
        self.state.clear_pending_install_approval();
        let _ = self.app.emit(UPDATE_INSTALL_PREFLIGHT_RELEASE_EVENT, ());
    }
}

fn require_self_update_policy() -> Result<(), UpdateCommandError> {
    match current_update_policy() {
        UpdatePolicy::SelfUpdate => Ok(()),
        UpdatePolicy::SystemManaged => Err(UpdateCommandError::new(
            "updates-managed",
            "Updates for this build are managed by your package manager.",
        )),
        UpdatePolicy::Disabled => Err(UpdateCommandError::new(
            "updates-disabled",
            "Updates are unavailable for this build.",
        )),
    }
}

fn current_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

fn automatic_checks_enabled(app: &AppHandle) -> bool {
    app.state::<AppStorage>()
        .get_setting(SETTING_AUTOMATIC_UPDATE_CHECKS)
        .map(|value| value.as_deref() != Some("false"))
        .unwrap_or(false)
}

fn status_from_check(source: &'static str, result: &UpdateCheckResponse) -> UpdateStatusSnapshot {
    match result {
        UpdateCheckResponse::UpToDate {
            message,
            current_version,
        } => UpdateStatusSnapshot::UpToDate {
            source,
            message: message.clone(),
            current_version: current_version.clone(),
        },
        UpdateCheckResponse::Available {
            message,
            current_version,
            version,
            published_at,
        } => UpdateStatusSnapshot::Available {
            source,
            message: message.clone(),
            current_version: current_version.clone(),
            version: version.clone(),
            published_at: published_at.clone(),
        },
    }
}

#[cfg(not(feature = "e2e"))]
fn build_updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, UpdateCommandError> {
    let builder = app.updater_builder();

    // Universal macOS archives use one stable metadata key regardless of the
    // architecture of the machine on which the app is running.
    #[cfg(target_os = "macos")]
    let builder = builder.target("macos-universal");

    builder.build().map_err(|error| {
        UpdateCommandError::new(
            "updater-configuration",
            format!("Failed to create updater client: {error}"),
        )
    })
}

async fn perform_update_check(app: &AppHandle) -> Result<UpdateCheckResponse, UpdateCommandError> {
    let version = current_version(&app);

    #[cfg(feature = "e2e")]
    {
        Ok(UpdateCheckResponse::UpToDate {
            message: "Grayslate is up to date.".to_string(),
            current_version: version,
        })
    }

    #[cfg(not(feature = "e2e"))]
    {
        let updater = build_updater(&app)?;
        let update = updater.check().await.map_err(|error| {
            UpdateCommandError::new(
                "check-failed",
                format!("Failed to check for updates: {error}"),
            )
        })?;

        match update {
            Some(update) => Ok(UpdateCheckResponse::Available {
                message: format!("Grayslate {} is available.", update.version),
                current_version: update.current_version,
                version: update.version,
                published_at: update.date.map(|date| date.to_string()),
            }),
            None => Ok(UpdateCheckResponse::UpToDate {
                message: "Grayslate is up to date.".to_string(),
                current_version: version,
            }),
        }
    }
}

#[tauri::command]
pub fn get_update_status(operations: State<'_, UpdateOperationState>) -> UpdateStatusSnapshot {
    operations.snapshot()
}

#[tauri::command]
pub async fn check_for_updates(
    app: AppHandle,
    operations: State<'_, UpdateOperationState>,
) -> Result<UpdateCheckResponse, UpdateCommandError> {
    require_self_update_policy()?;
    let _operation = operations.begin(UPDATE_CHECKING)?;
    operations.publish(&app, UpdateStatusSnapshot::Checking { source: "manual" });
    let result = perform_update_check(&app).await;
    match &result {
        Ok(response) => operations.publish(&app, status_from_check("manual", response)),
        Err(error) => operations.publish(
            &app,
            UpdateStatusSnapshot::Error {
                source: "manual",
                message: error.message.clone(),
            },
        ),
    }
    result
}

async fn run_automatic_update_check(app: &AppHandle) {
    if require_self_update_policy().is_err() || !automatic_checks_enabled(app) {
        return;
    }

    let operations = app.state::<UpdateOperationState>();
    let Ok(_operation) = operations.begin(UPDATE_CHECKING) else {
        return;
    };
    operations.publish(
        app,
        UpdateStatusSnapshot::Checking {
            source: "automatic",
        },
    );
    let result = perform_update_check(app).await;
    if !automatic_checks_enabled(app) {
        operations.publish(app, UpdateStatusSnapshot::Idle);
        return;
    }
    match result {
        Ok(response) => operations.publish(app, status_from_check("automatic", &response)),
        Err(error) => operations.publish(
            app,
            UpdateStatusSnapshot::Error {
                source: "automatic",
                message: error.message,
            },
        ),
    }
}

pub fn start_automatic_update_scheduler(app: AppHandle) {
    if require_self_update_policy().is_err() {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let scheduler = app.state::<AutomaticUpdateScheduler>();
        let mut delay = AUTOMATIC_UPDATE_STARTUP_DELAY;
        loop {
            let _ = tokio::time::timeout(delay, scheduler.wake.notified()).await;

            let operations = app.state::<UpdateOperationState>();
            if automatic_checks_enabled(&app) {
                if !operations.blocks_automatic_check() {
                    run_automatic_update_check(&app).await;
                }
            } else {
                operations.clear_automatic_status(&app);
            }
            delay = AUTOMATIC_UPDATE_INTERVAL;
        }
    });
}

#[cfg(not(feature = "e2e"))]
async fn request_install_approval(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    initiating_window_label: &str,
    operations: &UpdateOperationState,
) -> Result<bool, UpdateCommandError> {
    let window_label = window.label().to_string();
    let request_id = operations.begin_install_approval(&window_label)?;
    let payload = UpdateInstallPreflightPayload {
        request_id,
        restore_update_dialog: window_label == initiating_window_label,
    };
    if let Err(error) = window.emit(UPDATE_INSTALL_PREFLIGHT_EVENT, payload.clone()) {
        operations.clear_install_approval(&window_label, request_id);
        return Err(UpdateCommandError::new(
            "preflight-failed",
            format!("Could not ask window {window_label} to prepare for the update: {error}"),
        ));
    }

    let started_at = std::time::Instant::now();
    let mut last_emitted_at = started_at;
    loop {
        if let Some(allowed) = operations.take_install_approval_response(&window_label, request_id)
        {
            return Ok(allowed);
        }
        // A window that closed while being prompted no longer has document
        // state that the process-wide installer can discard.
        if app.get_webview_window(&window_label).is_none() {
            operations.clear_install_approval(&window_label, request_id);
            return Ok(true);
        }
        if started_at.elapsed() >= INSTALL_APPROVAL_TIMEOUT {
            operations.clear_install_approval(&window_label, request_id);
            return Err(UpdateCommandError::new(
                "preflight-timeout",
                format!(
                    "Window {window_label} did not respond to the update confirmation in time."
                ),
            ));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        if last_emitted_at.elapsed() >= INSTALL_APPROVAL_RETRY_INTERVAL {
            // A newly-created renderer may not have registered its listener
            // when the first event was sent. Reusing the same request token is
            // safe because the frontend de-duplicates it.
            let _ = window.emit(UPDATE_INSTALL_PREFLIGHT_EVENT, payload.clone());
            last_emitted_at = std::time::Instant::now();
        }
    }
}

#[cfg(not(feature = "e2e"))]
async fn prepare_all_windows_for_install(
    app: &AppHandle,
    initiating_window_label: &str,
    operations: &UpdateOperationState,
    autosave: &AutosaveRegistry,
) -> Result<HashMap<String, Option<ApprovedDocumentSnapshot>>, UpdateCommandError> {
    let mut windows = app.webview_windows().into_values().collect::<Vec<_>>();
    // Lock the initiating window first so the document behind the update
    // dialog cannot change while the user answers prompts in other windows.
    windows.sort_by_key(|window| {
        (
            window.label() != initiating_window_label,
            window.label().to_string(),
        )
    });

    let mut approved = HashMap::with_capacity(windows.len());
    for window in windows {
        let window_label = window.label().to_string();
        if !request_install_approval(app, &window, initiating_window_label, operations).await? {
            return Err(UpdateCommandError::new(
                "cancelled",
                "Update installation was cancelled. No windows were closed.",
            ));
        }
        approved.insert(
            window_label.clone(),
            document_snapshot(autosave, &window_label),
        );
    }
    Ok(approved)
}

#[cfg(not(feature = "e2e"))]
async fn flush_all_windows_before_install(
    app: &AppHandle,
    autosave: &AutosaveRegistry,
    documents: &DocumentRegistry,
    storage: &AppStorage,
    csv_registry: &CsvSessionRegistry,
    save_coordinator: &SaveCoordinator,
    approved: &HashMap<String, Option<ApprovedDocumentSnapshot>>,
) -> Result<(), UpdateCommandError> {
    validate_approved_local_documents(app, autosave, approved)?;
    for window in app.webview_windows().into_values() {
        flush_before_exit(
            app,
            &window,
            autosave,
            documents,
            storage,
            csv_registry,
            save_coordinator,
        )
        .await
        .map_err(|error| {
            UpdateCommandError::new(
                "save-failed",
                format!(
                    "Could not save window {} before installing: {error}",
                    window.label()
                ),
            )
        })?;
    }

    // Locks stop ordinary editor input, while this second validation also
    // fails closed if a native menu action or an unexpected renderer event
    // changed a local document during the final slate flushes.
    validate_approved_local_documents(app, autosave, approved)?;
    let dirty_slate = app.webview_windows().into_keys().find(|window_label| {
        autosave
            .get_document_info(window_label)
            .is_some_and(|document| document.source == FileSource::Slates && document.is_dirty)
    });
    if let Some(window_label) = dirty_slate {
        return Err(UpdateCommandError::new(
            "save-failed",
            format!(
                "Window {window_label} changed while update installation was being prepared. Installation was cancelled."
            ),
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn respond_update_install_preflight(
    window: tauri::Window,
    operations: State<'_, UpdateOperationState>,
    request_id: u64,
    allowed: bool,
) -> Result<(), String> {
    if !operations.is_installing() {
        return Err("No update installation is in progress.".to_string());
    }
    operations.respond_to_install_approval(window.label(), request_id, allowed)
}

#[tauri::command]
pub async fn install_available_update(
    app: AppHandle,
    window: tauri::WebviewWindow,
    operations: State<'_, UpdateOperationState>,
    autosave: State<'_, AutosaveRegistry>,
    documents: State<'_, DocumentRegistry>,
    storage: State<'_, AppStorage>,
    csv_registry: State<'_, CsvSessionRegistry>,
    save_coordinator: State<'_, SaveCoordinator>,
) -> Result<UpdateInstallResponse, UpdateCommandError> {
    require_self_update_policy()?;
    let available_status = operations.snapshot();
    let _operation = operations.begin(UPDATE_INSTALLING)?;
    let _preflight = UpdateInstallPreflightGuard {
        app: app.clone(),
        state: operations.inner(),
    };
    operations.publish(
        &app,
        UpdateStatusSnapshot::Installing {
            message: "Downloading and installing the update.".to_string(),
        },
    );

    let result: Result<UpdateInstallResponse, UpdateCommandError> = async {
        #[cfg(feature = "e2e")]
        {
            let _ = (
                &app,
                &window,
                &autosave,
                &documents,
                &storage,
                &csv_registry,
                &save_coordinator,
            );
            Err(UpdateCommandError::new(
                "no-update",
                "No update is currently available in the E2E fixture.",
            ))
        }

        #[cfg(not(feature = "e2e"))]
        {
            let updater = build_updater(&app)?;
            let update = updater
                .check()
                .await
                .map_err(|error| {
                    UpdateCommandError::new(
                        "check-failed",
                        format!("Failed to check for updates: {error}"),
                    )
                })?
                .ok_or_else(|| {
                    UpdateCommandError::new("no-update", "No update is currently available.")
                })?;

            let version = update.version.clone();
            let update_bytes = update.download(|_, _| {}, || {}).await.map_err(|error| {
                UpdateCommandError::new(
                    "download-failed",
                    format!("Failed to download update {version}: {error}"),
                )
            })?;
            // The Windows updater exits the whole process as soon as installation
            // begins. Every renderer must therefore approve that exit and become
            // read-only before any window's final managed-slate flush is accepted.
            let approved = prepare_all_windows_for_install(
                &app,
                window.label(),
                operations.inner(),
                autosave.inner(),
            )
            .await?;
            flush_all_windows_before_install(
                &app,
                autosave.inner(),
                documents.inner(),
                storage.inner(),
                csv_registry.inner(),
                save_coordinator.inner(),
                &approved,
            )
            .await
            ?;
            update.install(update_bytes).map_err(|error| {
                UpdateCommandError::new(
                    "install-failed",
                    format!("Failed to install update {version}: {error}"),
                )
            })?;

            Ok(UpdateInstallResponse {
                version: version.clone(),
                message: format!(
                    "Grayslate {version} has been installed. Restart the app when convenient to use the update."
                ),
            })
        }
    }
    .await;

    match &result {
        Ok(response) => operations.publish(
            &app,
            UpdateStatusSnapshot::Installed {
                version: response.version.clone(),
                message: response.message.clone(),
            },
        ),
        Err(error) if error.code == "cancelled" => operations.publish(&app, available_status),
        Err(error) => operations.publish(
            &app,
            UpdateStatusSnapshot::Error {
                source: "manual",
                message: error.message.clone(),
            },
        ),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_operations_are_mutually_exclusive_and_release_on_drop() {
        let state = UpdateOperationState::default();
        let guard = state
            .begin(UPDATE_CHECKING)
            .expect("first operation starts");
        let error = state
            .begin(UPDATE_INSTALLING)
            .err()
            .expect("concurrent operation is rejected");
        assert_eq!(error.code, "busy");

        drop(guard);
        assert!(state.begin(UPDATE_INSTALLING).is_ok());
    }

    #[test]
    #[cfg(not(feature = "e2e"))]
    fn install_approval_accepts_only_the_target_window_and_request() {
        let state = UpdateOperationState::default();
        let request_id = state
            .begin_install_approval("editor-one")
            .expect("approval request starts");

        assert!(state
            .respond_to_install_approval("editor-two", request_id, true)
            .is_err());
        assert!(state
            .respond_to_install_approval("editor-one", request_id + 1, true)
            .is_err());
        assert_eq!(
            state.take_install_approval_response("editor-one", request_id),
            None
        );

        state
            .respond_to_install_approval("editor-one", request_id, false)
            .expect("target window may respond");
        assert_eq!(
            state.take_install_approval_response("editor-one", request_id),
            Some(false)
        );
        assert_eq!(
            state.take_install_approval_response("editor-one", request_id),
            None
        );
    }

    #[test]
    fn installing_state_blocks_window_creation_only_while_guard_is_alive() {
        let state = UpdateOperationState::default();
        assert!(!state.is_installing());
        let guard = state
            .begin(UPDATE_INSTALLING)
            .expect("install operation starts");
        assert!(state.is_installing());
        drop(guard);
        assert!(!state.is_installing());
    }
}
