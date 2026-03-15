use std::env;

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;
use url::Url;

const UPDATER_ENDPOINTS_ENV: &str = "GRAYSLATE_UPDATER_ENDPOINTS";
const UPDATER_PUBKEY_ENV: &str = "GRAYSLATE_UPDATER_PUBKEY";

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

struct UpdaterConfig {
    endpoints: Vec<Url>,
    pubkey: String,
}

fn current_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

fn configured_endpoints() -> Result<Vec<Url>, String> {
    let raw = env::var(UPDATER_ENDPOINTS_ENV).map_err(|_| {
        format!(
            "Updates are not configured for this build. Set {} and {} for release builds.",
            UPDATER_ENDPOINTS_ENV, UPDATER_PUBKEY_ENV
        )
    })?;

    let endpoints: Result<Vec<_>, _> = raw
        .split(';')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            Url::parse(value)
                .map_err(|error| format!("Invalid updater endpoint '{}': {}", value, error))
        })
        .collect();

    let endpoints = endpoints?;
    if endpoints.is_empty() {
        return Err(format!(
            "Updates are not configured for this build. Set {} and {} for release builds.",
            UPDATER_ENDPOINTS_ENV, UPDATER_PUBKEY_ENV
        ));
    }

    Ok(endpoints)
}

fn updater_config() -> Result<UpdaterConfig, String> {
    let endpoints = configured_endpoints()?;
    let pubkey = env::var(UPDATER_PUBKEY_ENV).map_err(|_| {
        format!(
            "Updates are not configured for this build. Set {} and {} for release builds.",
            UPDATER_ENDPOINTS_ENV, UPDATER_PUBKEY_ENV
        )
    })?;

    if pubkey.trim().is_empty() {
        return Err(format!(
            "Updates are not configured for this build. Set {} and {} for release builds.",
            UPDATER_ENDPOINTS_ENV, UPDATER_PUBKEY_ENV
        ));
    }

    Ok(UpdaterConfig { endpoints, pubkey })
}

fn build_updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    let config = updater_config()?;
    let builder = app
        .updater_builder()
        .pubkey(config.pubkey)
        .endpoints(config.endpoints)
        .map_err(|error| format!("Failed to configure updater endpoints: {}", error))?;

    builder
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
