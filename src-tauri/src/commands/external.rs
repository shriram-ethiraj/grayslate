use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;
use url::Url;

use crate::{
    document::{revalidate_source_authority, DocumentAccess, DocumentRegistry},
    storage::AppStorage,
    update_policy::{current_update_policy, UpdatePolicy},
};

const MAX_LINK_LENGTH: usize = 8 * 1024;
const MAX_RELEASE_VERSION_LENGTH: usize = 128;
const REPOSITORY_URL: &str = "https://github.com/shriram-ethiraj/grayslate";
const RELEASES_URL: &str = "https://github.com/shriram-ethiraj/grayslate/releases";
const LICENSE_URL: &str = "https://github.com/shriram-ethiraj/grayslate/blob/main/LICENSE";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    app_name: String,
    app_version: String,
    update_policy: UpdatePolicy,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AboutLinkTarget {
    License,
    Releases,
    ReleaseNotes,
    Repository,
}

enum MarkdownDestination {
    Url(String),
    Path(PathBuf),
}

#[tauri::command]
pub fn get_app_info(app: tauri::AppHandle) -> AppInfo {
    AppInfo {
        app_name: app.package_info().name.clone(),
        app_version: app.package_info().version.to_string(),
        update_policy: current_update_policy(),
    }
}

#[tauri::command]
pub fn open_about_link(
    app: tauri::AppHandle,
    target: AboutLinkTarget,
    version: Option<String>,
) -> Result<(), String> {
    let destination = about_link_destination(target, version.as_deref())?;
    app.opener()
        .open_url(destination, None::<&str>)
        .map_err(|error| format!("Failed to open project link: {error}"))
}

#[tauri::command]
pub async fn open_markdown_link(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    href: String,
    document_id: Option<String>,
    document_generation: Option<u64>,
) -> Result<(), String> {
    validate_display_input(&href)?;

    let destination = match parse_markdown_url(&href)? {
        Some(url) => MarkdownDestination::Url(url),
        None => {
            let id = document_id.as_deref().ok_or_else(|| {
                "Save the Markdown file before opening a relative link.".to_string()
            })?;
            let generation = document_generation
                .ok_or_else(|| "Markdown document authorization is missing.".to_string())?;
            let document =
                documents.resolve(window.label(), id, generation, DocumentAccess::Read)?;
            revalidate_source_authority(&app, storage.inner(), &document)?;
            MarkdownDestination::Path(resolve_markdown_file(&document.path, &href)?)
        }
    };

    let display_destination = match &destination {
        MarkdownDestination::Url(url) => url.clone(),
        MarkdownDestination::Path(path) => path
            .to_str()
            .ok_or_else(|| "The linked file path is not valid UTF-8.".to_string())?
            .to_string(),
    };

    let confirmed = confirm_markdown_destination(&app, &window, display_destination).await?;
    if !confirmed {
        return Ok(());
    }

    match destination {
        MarkdownDestination::Url(url) => app
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|error| format!("Failed to open Markdown link: {error}")),
        MarkdownDestination::Path(path) => {
            revalidate_linked_file(&path)?;
            let path = path
                .into_os_string()
                .into_string()
                .map_err(|_| "The linked file path is not valid UTF-8.".to_string())?;
            app.opener()
                .open_path(path, None::<&str>)
                .map_err(|error| format!("Failed to open linked file: {error}"))
        }
    }
}

fn about_link_destination(
    target: AboutLinkTarget,
    version: Option<&str>,
) -> Result<String, String> {
    match target {
        AboutLinkTarget::License => Ok(LICENSE_URL.to_string()),
        AboutLinkTarget::Releases => Ok(RELEASES_URL.to_string()),
        AboutLinkTarget::Repository => Ok(REPOSITORY_URL.to_string()),
        AboutLinkTarget::ReleaseNotes => {
            let version = version.ok_or_else(|| "A release version is required.".to_string())?;
            if version.is_empty()
                || version.len() > MAX_RELEASE_VERSION_LENGTH
                || !version.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+')
                })
            {
                return Err("Release version is invalid.".to_string());
            }

            let tag = if version.starts_with('v') {
                version.to_string()
            } else {
                format!("v{version}")
            };
            let mut url = Url::parse(RELEASES_URL)
                .map_err(|_| "The configured releases URL is invalid.".to_string())?;
            url.path_segments_mut()
                .map_err(|_| "The configured releases URL cannot be extended.".to_string())?
                .extend(["tag", tag.as_str()]);
            Ok(url.into())
        }
    }
}

fn validate_display_input(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_LINK_LENGTH {
        return Err("Markdown link is empty or too long.".to_string());
    }
    if value.chars().any(|character| {
        character.is_control()
            || matches!(
                character,
                '\u{200e}'
                    | '\u{200f}'
                    | '\u{202a}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            )
    }) {
        return Err("Markdown link contains unsafe display characters.".to_string());
    }
    Ok(())
}

fn parse_markdown_url(href: &str) -> Result<Option<String>, String> {
    let trimmed = href.trim();
    let candidate = if trimmed.starts_with("//") {
        format!("https:{trimmed}")
    } else {
        trimmed.to_string()
    };

    match Url::parse(&candidate) {
        Ok(url) => {
            if !matches!(url.scheme(), "ftp" | "http" | "https" | "mailto" | "tel") {
                return Err("This Markdown link type is not supported.".to_string());
            }
            Ok(Some(url.into()))
        }
        Err(url::ParseError::RelativeUrlWithoutBase) => Ok(None),
        Err(_) => Err("Markdown link URL is invalid.".to_string()),
    }
}

fn resolve_markdown_file(document_path: &Path, href: &str) -> Result<PathBuf, String> {
    let path_end = href.find(['?', '#']).unwrap_or(href.len());
    let encoded_path = href[..path_end].trim();
    if encoded_path.is_empty() {
        return Err("The relative Markdown link is empty.".to_string());
    }
    let decoded = urlencoding::decode(encoded_path)
        .map_err(|_| "The relative Markdown link has invalid encoding.".to_string())?
        .replace('\\', "/");
    let relative_path = Path::new(&decoded);
    if relative_path.is_absolute() {
        return Err("Markdown file links must be relative to the current document.".to_string());
    }

    let parent = document_path
        .parent()
        .ok_or_else(|| "The Markdown document has no parent directory.".to_string())?;
    let candidate = parent.join(relative_path);
    let metadata = std::fs::symlink_metadata(&candidate)
        .map_err(|error| format!("Cannot inspect linked file: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Markdown links may open only regular, non-symlink files.".to_string());
    }
    std::fs::canonicalize(candidate).map_err(|error| format!("Cannot resolve linked file: {error}"))
}

fn revalidate_linked_file(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("Cannot revalidate linked file: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("The linked file changed before it could be opened.".to_string());
    }
    let canonical = std::fs::canonicalize(path)
        .map_err(|error| format!("Cannot revalidate linked file: {error}"))?;
    if canonical != path {
        return Err("The linked file changed before it could be opened.".to_string());
    }
    Ok(())
}

async fn confirm_markdown_destination(
    app: &tauri::AppHandle,
    window: &tauri::Window,
    destination: String,
) -> Result<bool, String> {
    let dialog_app = app.clone();
    let dialog_window = window.clone();
    tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .message(format!(
                "Open this destination outside Grayslate?\n\n{destination}"
            ))
            .title("Open Markdown link")
            .parent(&dialog_window)
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Open".to_string(),
                "Cancel".to_string(),
            ))
            .blocking_show()
    })
    .await
    .map_err(|error| format!("Failed to join Markdown confirmation task: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_links_are_closed_over_known_destinations() {
        assert_eq!(
            about_link_destination(AboutLinkTarget::Repository, None).unwrap(),
            REPOSITORY_URL
        );
        assert!(about_link_destination(AboutLinkTarget::ReleaseNotes, Some("../../bad")).is_err());
        assert!(
            about_link_destination(AboutLinkTarget::ReleaseNotes, Some("1.2.3"))
                .unwrap()
                .ends_with("/tag/v1.2.3")
        );
    }

    #[test]
    fn markdown_urls_allow_only_explicit_external_schemes() {
        assert_eq!(
            parse_markdown_url("//example.com/docs").unwrap().as_deref(),
            Some("https://example.com/docs")
        );
        assert!(parse_markdown_url("javascript:alert(1)").is_err());
        assert!(parse_markdown_url("file:///tmp/secret").is_err());
        assert_eq!(parse_markdown_url("guide/setup.md").unwrap(), None);
    }

    #[test]
    fn unsafe_confirmation_display_characters_are_rejected() {
        assert!(validate_display_input("https://safe.example/path").is_ok());
        assert!(validate_display_input("https://safe.example/\nhttps://evil.example").is_err());
        assert!(validate_display_input("https://example.com/\u{202e}moc.live").is_err());
    }
}
