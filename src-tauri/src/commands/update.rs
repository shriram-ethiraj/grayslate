use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum UpdateCheckResponse {
    Unconfigured {
        message: String,
        current_version: String,
    },
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

fn current_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

fn build_updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    app.updater_builder()
        .build()
        .map_err(|error| format!("Failed to create updater client: {}", error))
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateCheckResponse, String> {
    let version = current_version(&app);
    let updater = match build_updater(&app) {
        Ok(updater) => updater,
        Err(message) => {
            return Ok(UpdateCheckResponse::Unconfigured {
                message,
                current_version: version,
            })
        }
    };

    let update = updater
        .check()
        .await
        .map_err(|error| format!("Failed to check for updates: {}", error))?;

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
pub async fn install_available_update(app: AppHandle) -> Result<UpdateInstallResponse, String> {
    let updater = build_updater(&app)?;
    let update = updater
        .check()
        .await
        .map_err(|error| format!("Failed to check for updates: {}", error))?
        .ok_or_else(|| "No update is currently available.".to_string())?;

    let version = update.version.clone();
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| format!("Failed to install update {}: {}", version, error))?;

    Ok(UpdateInstallResponse {
        version: version.clone(),
        message: format!(
            "Grayslate {} has been installed. Restart the app when convenient to use the update.",
            version
        ),
    })
}
