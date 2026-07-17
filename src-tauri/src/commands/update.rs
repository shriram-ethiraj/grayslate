use std::sync::atomic::{AtomicU8, Ordering};

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_updater::UpdaterExt;

use crate::update_policy::{current_update_policy, UpdatePolicy};

const UPDATE_IDLE: u8 = 0;
const UPDATE_CHECKING: u8 = 1;
const UPDATE_INSTALLING: u8 = 2;

#[derive(Serialize)]
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

#[derive(Serialize)]
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

#[tauri::command]
pub async fn check_for_updates(
    app: AppHandle,
    operations: State<'_, UpdateOperationState>,
) -> Result<UpdateCheckResponse, UpdateCommandError> {
    require_self_update_policy()?;
    let _operation = operations.begin(UPDATE_CHECKING)?;
    let version = current_version(&app);
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

#[tauri::command]
pub async fn install_available_update(
    app: AppHandle,
    operations: State<'_, UpdateOperationState>,
) -> Result<UpdateInstallResponse, UpdateCommandError> {
    require_self_update_policy()?;
    let _operation = operations.begin(UPDATE_INSTALLING)?;
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
        .ok_or_else(|| UpdateCommandError::new("no-update", "No update is currently available."))?;

    let version = update.version.clone();
    let update_bytes = update.download(|_, _| {}, || {}).await.map_err(|error| {
        UpdateCommandError::new(
            "download-failed",
            format!("Failed to download update {version}: {error}"),
        )
    })?;
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
