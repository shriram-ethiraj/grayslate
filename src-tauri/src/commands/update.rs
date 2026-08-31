use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Mutex,
    },
    time::Duration,
};

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

const UPDATE_IDLE: u8 = 0;
const UPDATE_CHECKING: u8 = 1;
const UPDATE_INSTALLING: u8 = 2;
const AUTOMATIC_UPDATE_STARTUP_DELAY: Duration = Duration::from_secs(5);
const AUTOMATIC_UPDATE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
pub const UPDATE_STATUS_EVENT: &str = "updates://status";

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

impl AutomaticUpdateScheduler {
    pub fn settings_changed(&self) {
        self.wake.notify_one();
    }
}

struct UpdateOperationGuard<'a> {
    state: &'a UpdateOperationState,
}

impl Drop for UpdateOperationGuard<'_> {
    fn drop(&mut self) {
        self.state.operation.store(UPDATE_IDLE, Ordering::Release);
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

#[tauri::command]
pub async fn install_available_update(
    app: AppHandle,
    window: tauri::Window,
    operations: State<'_, UpdateOperationState>,
    autosave: State<'_, AutosaveRegistry>,
    documents: State<'_, DocumentRegistry>,
    storage: State<'_, AppStorage>,
    csv_registry: State<'_, CsvSessionRegistry>,
    save_coordinator: State<'_, SaveCoordinator>,
) -> Result<UpdateInstallResponse, UpdateCommandError> {
    require_self_update_policy()?;
    let _operation = operations.begin(UPDATE_INSTALLING)?;
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
            // The Windows updater exits the process as soon as installation begins.
            // Flush the managed slate at the last possible moment; a failure must keep
            // the current app running instead of trading user data for an update.
            flush_before_exit(
                &app,
                &window,
                autosave.inner(),
                documents.inner(),
                storage.inner(),
                csv_registry.inner(),
                save_coordinator.inner(),
            )
            .await
            .map_err(|error| UpdateCommandError::new("save-failed", error))?;
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
}
