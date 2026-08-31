use std::{
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::Serialize;
use tauri::http::HeaderValue;
use tauri::{Manager, WebviewWindow};
use url::Url;
use uuid::Uuid;

use crate::document::{DocumentAccess, DocumentDescriptor, DocumentRegistry};
use crate::storage::{AppStorage, SETTING_SIDEBAR_OPEN, SETTING_SIDEBAR_WIDTH, SETTING_THEME};

const PERMISSIONS_POLICY: &str =
    "camera=(), microphone=(), geolocation=(), display-capture=(), usb=(), serial=(), hid=(), payment=()";

const SECONDARY_WINDOW_PREFIX: &str = "editor-";
const WINDOW_CASCADE_OFFSET: i32 = 28;
const NEW_SLATE_WINDOW_TITLE: &str = "New Slate";

#[derive(Clone, Debug)]
struct OpenReservation {
    window_label: String,
    path: PathBuf,
}

#[derive(Clone, Debug)]
enum StoredLaunchIntent {
    Blank,
    Document {
        document: DocumentDescriptor,
        reservation_id: String,
    },
}

#[derive(Default)]
struct WindowRegistryState {
    focused_window: Option<String>,
    primary_window: Option<String>,
    live_windows: Vec<String>,
    focus_order: Vec<String>,
    sidebar_layouts: HashMap<String, SidebarLayout>,
    launch_intents: HashMap<String, StoredLaunchIntent>,
    active_paths: HashMap<String, PathBuf>,
    path_owners: HashMap<PathBuf, String>,
    reservations: HashMap<String, OpenReservation>,
    reserved_paths: HashMap<PathBuf, String>,
}

#[derive(Clone, Debug)]
pub struct SidebarLayout {
    pub width: String,
    pub open: String,
}

#[derive(Clone, Debug)]
pub struct PrimaryWindowPromotion {
    pub window_label: String,
    pub sidebar_layout: Option<SidebarLayout>,
}

/// Process-wide coordination for native windows and exclusive document ownership.
///
/// Frontend module state remains isolated in each webview. This registry owns the
/// small cross-window facts that must be shared: primary-window election, focus,
/// inherited sidebar layout, one-shot launch payloads, and canonical-path ownership.
#[derive(Default)]
pub struct WindowRegistry {
    inner: Mutex<WindowRegistryState>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum OpenDisposition {
    #[serde(rename_all = "camelCase")]
    OpenHere {
        reservation_id: String,
    },
    FocusedExisting,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CreateWindowResult {
    #[serde(rename_all = "camelCase")]
    Created {
        window_label: String,
    },
    FocusedExisting,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WindowLaunchIntent {
    PrimaryStartup,
    Blank,
    #[serde(rename_all = "camelCase")]
    Document {
        document: DocumentDescriptor,
        reservation_id: String,
    },
}

impl WindowRegistry {
    pub fn register_window(&self, window_label: &str, inherit_from: Option<&str>) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !state.live_windows.iter().any(|label| label == window_label) {
            state.live_windows.push(window_label.to_string());
        }
        if state.primary_window.is_none() {
            state.primary_window = Some(window_label.to_string());
        }
        if let Some(layout) =
            inherit_from.and_then(|source| state.sidebar_layouts.get(source).cloned())
        {
            state
                .sidebar_layouts
                .insert(window_label.to_string(), layout);
        }
    }

    pub fn note_focused(&self, window_label: &str) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.focused_window = Some(window_label.to_string());
        state.focus_order.retain(|label| label != window_label);
        state.focus_order.push(window_label.to_string());
    }

    pub fn is_primary(&self, window_label: &str) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .primary_window
            .as_deref()
            == Some(window_label)
    }

    pub fn resolve_sidebar_layout(
        &self,
        window_label: &str,
        default_width: String,
        default_open: String,
    ) -> SidebarLayout {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state
            .sidebar_layouts
            .entry(window_label.to_string())
            .or_insert_with(|| SidebarLayout {
                width: default_width,
                open: default_open,
            })
            .clone()
    }

    /// Record every window's local layout, but authorize persistent writes only
    /// for the backend-elected primary window.
    pub fn note_sidebar_setting(&self, window_label: &str, key: &str, value: Option<&str>) -> bool {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let layout = state
            .sidebar_layouts
            .entry(window_label.to_string())
            .or_insert_with(|| SidebarLayout {
                width: "20".to_string(),
                open: "false".to_string(),
            });
        match (key, value) {
            (SETTING_SIDEBAR_WIDTH, Some(width)) => layout.width = width.to_string(),
            (SETTING_SIDEBAR_OPEN, Some(open)) => layout.open = open.to_string(),
            _ => {}
        }
        state.primary_window.as_deref() == Some(window_label)
    }

    pub fn preferred_window_label(&self, app: &tauri::AppHandle) -> Option<String> {
        let focused = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .focused_window
            .clone();
        if focused
            .as_deref()
            .is_some_and(|label| app.get_webview_window(label).is_some())
        {
            return focused;
        }
        let primary = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .primary_window
            .clone();
        if primary
            .as_deref()
            .is_some_and(|label| app.get_webview_window(label).is_some())
        {
            return primary;
        }
        app.webview_windows().keys().next().cloned()
    }

    fn reserve(&self, window_label: &str, path: &Path) -> Result<String, String> {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if let Some(owner) = state.path_owners.get(path) {
            if owner != window_label {
                return Err(owner.clone());
            }
        }
        if let Some(reservation_id) = state.reserved_paths.get(path) {
            if let Some(reservation) = state.reservations.get(reservation_id) {
                if reservation.window_label != window_label {
                    return Err(reservation.window_label.clone());
                }
            }
        }

        // A superseded open in the same window must not retain its old claim.
        let stale_ids = state
            .reservations
            .iter()
            .filter_map(|(id, reservation)| {
                (reservation.window_label == window_label).then(|| id.clone())
            })
            .collect::<Vec<_>>();
        for id in stale_ids {
            if let Some(stale) = state.reservations.remove(&id) {
                state.reserved_paths.remove(&stale.path);
            }
        }

        let reservation_id = Uuid::now_v7().to_string();
        state
            .reserved_paths
            .insert(path.to_path_buf(), reservation_id.clone());
        state.reservations.insert(
            reservation_id.clone(),
            OpenReservation {
                window_label: window_label.to_string(),
                path: path.to_path_buf(),
            },
        );
        Ok(reservation_id)
    }

    pub fn commit_open(
        &self,
        window_label: &str,
        reservation_id: &str,
        path: &Path,
    ) -> Result<(), String> {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let reservation = state
            .reservations
            .get(reservation_id)
            .ok_or_else(|| "Document open authorization is invalid or expired.".to_string())?;
        if reservation.window_label != window_label || reservation.path != path {
            return Err("Document open authorization is invalid or expired.".to_string());
        }
        if state
            .path_owners
            .get(path)
            .is_some_and(|owner| owner != window_label)
        {
            return Err("The file is already open in another Grayslate window.".to_string());
        }

        let Some(reservation) = state.reservations.remove(reservation_id) else {
            return Err("Document open authorization is invalid or expired.".to_string());
        };
        state.reserved_paths.remove(&reservation.path);
        if let Some(previous_path) = state
            .active_paths
            .insert(window_label.to_string(), path.to_path_buf())
        {
            state.path_owners.remove(&previous_path);
        }
        state
            .path_owners
            .insert(path.to_path_buf(), window_label.to_string());
        Ok(())
    }

    pub fn cancel_open(&self, window_label: &str, reservation_id: &str) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let should_remove = state
            .reservations
            .get(reservation_id)
            .is_some_and(|reservation| reservation.window_label == window_label);
        if should_remove {
            if let Some(reservation) = state.reservations.remove(reservation_id) {
                state.reserved_paths.remove(&reservation.path);
            }
        }
    }

    pub fn release_active(&self, window_label: &str) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(path) = state.active_paths.remove(window_label) {
            state.path_owners.remove(&path);
        }
    }

    pub fn release_path(&self, window_label: &str, path: &Path) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.active_paths.get(window_label).map(PathBuf::as_path) == Some(path) {
            state.active_paths.remove(window_label);
            state.path_owners.remove(path);
        }
    }

    pub fn owner_for_path(&self, path: &Path) -> Option<String> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .path_owners
            .get(path)
            .cloned()
    }

    pub fn adopt_active_path(&self, window_label: &str, path: &Path) -> Result<(), String> {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(owner) = state.path_owners.get(path) {
            if owner != window_label {
                return Err(owner.clone());
            }
        }
        if let Some(previous_path) = state
            .active_paths
            .insert(window_label.to_string(), path.to_path_buf())
        {
            state.path_owners.remove(&previous_path);
        }
        state
            .path_owners
            .insert(path.to_path_buf(), window_label.to_string());
        Ok(())
    }

    pub fn replace_active_path(&self, window_label: &str, old_path: &Path, new_path: &Path) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.active_paths.get(window_label).map(PathBuf::as_path) != Some(old_path) {
            return;
        }
        state.path_owners.remove(old_path);
        state
            .active_paths
            .insert(window_label.to_string(), new_path.to_path_buf());
        state
            .path_owners
            .insert(new_path.to_path_buf(), window_label.to_string());
    }

    pub fn cleanup_window(&self, window_label: &str) -> Option<PrimaryWindowPromotion> {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.launch_intents.remove(window_label);
        if let Some(path) = state.active_paths.remove(window_label) {
            state.path_owners.remove(&path);
        }
        let reservation_ids = state
            .reservations
            .iter()
            .filter_map(|(id, reservation)| {
                (reservation.window_label == window_label).then(|| id.clone())
            })
            .collect::<Vec<_>>();
        for id in reservation_ids {
            if let Some(reservation) = state.reservations.remove(&id) {
                state.reserved_paths.remove(&reservation.path);
            }
        }
        if state.focused_window.as_deref() == Some(window_label) {
            state.focused_window = None;
        }
        state.live_windows.retain(|label| label != window_label);
        state.focus_order.retain(|label| label != window_label);
        state.sidebar_layouts.remove(window_label);

        if state.primary_window.as_deref() != Some(window_label) {
            return None;
        }

        let next_primary = state
            .focus_order
            .last()
            .cloned()
            .or_else(|| state.live_windows.last().cloned());
        state.primary_window = next_primary.clone();
        next_primary.map(|window_label| PrimaryWindowPromotion {
            sidebar_layout: state.sidebar_layouts.get(&window_label).cloned(),
            window_label,
        })
    }
}

pub fn focus_window(app: &tauri::AppHandle, window_label: &str) {
    if let Some(window) = app.get_webview_window(window_label) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn focus_preferred_window(app: &tauri::AppHandle) {
    if let Some(registry) = app.try_state::<WindowRegistry>() {
        if let Some(label) = registry.preferred_window_label(app) {
            focus_window(app, &label);
        }
    }
}

#[cfg(target_os = "macos")]
pub fn reopen_or_create_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(registry) = app.try_state::<WindowRegistry>() {
        if let Some(label) = registry.preferred_window_label(app) {
            focus_window(app, &label);
            return Ok(());
        }
    }

    let config = main_window_config(app)?;
    let window = build_configured_window(app, config)?;
    apply_macos_window_styling_to(&window);
    show_window(&window)?;
    Ok(())
}

fn main_window_config(
    app: &tauri::AppHandle,
) -> Result<tauri::utils::config::WindowConfig, String> {
    app.config()
        .app
        .windows
        .iter()
        .find(|config| config.label == "main")
        .cloned()
        .ok_or_else(|| "Main window configuration is missing.".to_string())
}

fn build_configured_window(
    app: &tauri::AppHandle,
    mut config: tauri::utils::config::WindowConfig,
) -> Result<WebviewWindow, String> {
    let dev_url = if cfg!(debug_assertions) {
        app.config().build.dev_url.clone()
    } else {
        None
    };
    let use_https_scheme = config.use_https_scheme;
    config.visible = false;
    config.background_color = Some(
        match app
            .try_state::<AppStorage>()
            .and_then(|storage| storage.get_setting(SETTING_THEME).ok().flatten())
            .as_deref()
        {
            Some("light") => tauri::utils::config::Color(240, 242, 247, 255),
            _ => tauri::utils::config::Color(27, 30, 38, 255),
        },
    );

    tauri::WebviewWindowBuilder::from_config(app, &config)
        .map_err(|error| error.to_string())?
        .on_navigation(move |url| {
            let allowed = is_allowed_app_navigation(url, dev_url.as_ref(), use_https_scheme);
            #[cfg(feature = "e2e")]
            crate::commands::e2e::record_navigation_decision("navigation", url.as_str(), allowed);
            allowed
        })
        .on_new_window(|_url, _| {
            #[cfg(feature = "e2e")]
            crate::commands::e2e::record_navigation_decision("new-window", _url.as_str(), false);
            tauri::webview::NewWindowResponse::Deny
        })
        .on_web_resource_request(|_, response| {
            response.headers_mut().insert(
                "Permissions-Policy",
                HeaderValue::from_static(PERMISSIONS_POLICY),
            );
        })
        .build()
        .map_err(|error| error.to_string())
}

fn create_secondary_window(
    app: &tauri::AppHandle,
    source_window: &tauri::Window,
    window_label: &str,
    title: &str,
) -> Result<WebviewWindow, String> {
    let mut config = main_window_config(app)?;
    config.label = window_label.to_string();
    config.title = title.to_string();
    let window = build_configured_window(app, config)?;
    app.state::<WindowRegistry>()
        .register_window(window_label, Some(source_window.label()));

    if let Ok(size) = source_window.inner_size() {
        let _ = window.set_size(size);
    }
    if let Ok(position) = source_window.outer_position() {
        let cascaded = tauri::PhysicalPosition::new(
            position.x.saturating_add(WINDOW_CASCADE_OFFSET),
            position.y.saturating_add(WINDOW_CASCADE_OFFSET),
        );
        let _ = window.set_position(cascaded);
    }

    #[cfg(target_os = "macos")]
    apply_macos_window_styling_to(&window);

    show_window(&window)?;

    Ok(window)
}

fn show_window(window: &WebviewWindow) -> Result<(), String> {
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_editor_window(
    app: tauri::AppHandle,
    window: tauri::Window,
    registry: tauri::State<'_, WindowRegistry>,
    documents: tauri::State<'_, DocumentRegistry>,
    document_id: Option<String>,
    document_generation: Option<u64>,
) -> Result<CreateWindowResult, String> {
    let source_document = match (document_id.as_deref(), document_generation) {
        (None, None) => None,
        (Some(id), Some(generation)) => {
            Some(documents.resolve(window.label(), id, generation, DocumentAccess::Read)?)
        }
        _ => return Err("Document ID and generation must be provided together.".to_string()),
    };

    if let Some(document) = source_document.as_ref() {
        if let Some(owner) = registry.owner_for_path(&document.path) {
            focus_window(&app, &owner);
            return Ok(CreateWindowResult::FocusedExisting);
        }
    }

    let window_label = format!("{SECONDARY_WINDOW_PREFIX}{}", Uuid::now_v7());
    let (intent, title) = if let Some(document) = source_document {
        let reservation_id = match registry.reserve(&window_label, &document.path) {
            Ok(reservation_id) => reservation_id,
            Err(owner) => {
                focus_window(&app, &owner);
                return Ok(CreateWindowResult::FocusedExisting);
            }
        };
        let target_document = match documents.grant_existing(
            &window_label,
            &document.path,
            document.source,
            document.rights,
        ) {
            Ok(document) => document,
            Err(error) => {
                let _ = registry.cleanup_window(&window_label);
                documents.revoke_window(&window_label);
                return Err(error);
            }
        };
        let descriptor = target_document.descriptor();
        let title = format_window_title(&descriptor.file_name, false);
        (
            StoredLaunchIntent::Document {
                document: descriptor,
                reservation_id,
            },
            title,
        )
    } else {
        (
            StoredLaunchIntent::Blank,
            NEW_SLATE_WINDOW_TITLE.to_string(),
        )
    };

    registry
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .launch_intents
        .insert(window_label.clone(), intent);

    if let Err(error) = create_secondary_window(&app, &window, &window_label, &title) {
        let _ = registry.cleanup_window(&window_label);
        documents.revoke_window(&window_label);
        return Err(error);
    }

    Ok(CreateWindowResult::Created { window_label })
}

#[tauri::command]
pub fn take_window_launch_intent(
    window: tauri::Window,
    registry: tauri::State<'_, WindowRegistry>,
) -> WindowLaunchIntent {
    let intent = registry
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .launch_intents
        .remove(window.label());
    match intent {
        Some(StoredLaunchIntent::Blank) => WindowLaunchIntent::Blank,
        Some(StoredLaunchIntent::Document {
            document,
            reservation_id,
        }) => WindowLaunchIntent::Document {
            document,
            reservation_id,
        },
        None if window.label() == "main" => WindowLaunchIntent::PrimaryStartup,
        None => WindowLaunchIntent::Blank,
    }
}

#[tauri::command]
pub fn claim_document_open(
    app: tauri::AppHandle,
    window: tauri::Window,
    registry: tauri::State<'_, WindowRegistry>,
    documents: tauri::State<'_, DocumentRegistry>,
    document_id: String,
    document_generation: u64,
) -> Result<OpenDisposition, String> {
    let document = documents.resolve(
        window.label(),
        &document_id,
        document_generation,
        DocumentAccess::Read,
    )?;
    match registry.reserve(window.label(), &document.path) {
        Ok(reservation_id) => Ok(OpenDisposition::OpenHere { reservation_id }),
        Err(owner) => {
            focus_window(&app, &owner);
            Ok(OpenDisposition::FocusedExisting)
        }
    }
}

#[tauri::command]
pub fn cancel_document_open(
    window: tauri::Window,
    registry: tauri::State<'_, WindowRegistry>,
    reservation_id: String,
) {
    registry.cancel_open(window.label(), &reservation_id);
}

fn sanitize_title_part(value: &str) -> String {
    let sanitized = value
        .chars()
        .filter(|character| !character.is_control())
        .take(160)
        .collect::<String>();
    if sanitized.trim().is_empty() {
        "New Slate".to_string()
    } else {
        sanitized
    }
}

fn format_window_title(file_name: &str, dirty: bool) -> String {
    let dirty_prefix = if dirty { "● " } else { "" };
    format!(
        "{dirty_prefix}{} — Grayslate",
        sanitize_title_part(file_name)
    )
}

#[tauri::command]
pub fn sync_window_title(
    window: tauri::Window,
    documents: tauri::State<'_, DocumentRegistry>,
    document_id: Option<String>,
    document_generation: Option<u64>,
    dirty: bool,
) -> Result<(), String> {
    let title = match (document_id.as_deref(), document_generation) {
        (None, None) => NEW_SLATE_WINDOW_TITLE.to_string(),
        (Some(id), Some(generation)) => {
            let document =
                documents.resolve(window.label(), id, generation, DocumentAccess::Read)?;
            let descriptor = document.descriptor();
            format_window_title(&descriptor.file_name, dirty && descriptor.source == "local")
        }
        _ => return Err("Document ID and generation must be provided together.".to_string()),
    };
    window.set_title(&title).map_err(|error| error.to_string())
}

/// Create the configured main window with fail-closed navigation hooks.
///
/// Tauri can only attach `on_new_window` while a webview is being built, so
/// `tauri.conf.json` sets `create: false` and this function recreates that same
/// configured window during setup. External links must continue to use the
/// validated Rust opener commands; the application webview itself never
/// navigates away and never creates child webviews.
pub fn create_main_window(app: &tauri::App) -> Result<(), Box<dyn Error>> {
    let config = main_window_config(app.handle()).map_err(std::io::Error::other)?;
    let window = build_configured_window(app.handle(), config).map_err(std::io::Error::other)?;
    app.state::<WindowRegistry>()
        .register_window(window.label(), None);

    #[cfg(target_os = "macos")]
    apply_macos_window_styling_to(&window);

    show_window(&window).map_err(std::io::Error::other)?;

    Ok(())
}

/// Allow only Grayslate's bundled application origin and the exact Vite dev
/// origin. Matching a full origin (scheme, host, and effective port) prevents a
/// compromised renderer from treating arbitrary localhost services as trusted.
fn is_allowed_app_navigation(url: &Url, dev_url: Option<&Url>, use_https_scheme: bool) -> bool {
    let is_bundled_origin =
        (url.scheme() == "tauri" && url.host_str() == Some("localhost") && url.port().is_none())
            || (url.scheme() == if use_https_scheme { "https" } else { "http" }
                && url.host_str() == Some("tauri.localhost")
                && url.port().is_none());

    is_bundled_origin || dev_url.is_some_and(|allowed| has_same_origin(url, allowed))
}

fn has_same_origin(candidate: &Url, allowed: &Url) -> bool {
    candidate.scheme() == allowed.scheme()
        && candidate.host_str() == allowed.host_str()
        && candidate.port_or_known_default() == allowed.port_or_known_default()
}

/// Apply macOS-specific window styling: rounded corners + shadow.
///
/// Decorations, titleBarStyle and trafficLightPosition are now set
/// declaratively in `tauri.macos.conf.json` (platform-specific config),
/// so this function only applies visual tweaks that require native APIs:
///   • transparent NSWindow background  → rounded corners show through
///   • CALayer corner radius            → clips web content to rounded rect
///   • system shadow                    → native drop-shadow (like Chrome)
///
/// Uses `objc2` + `objc2-app-kit` + `objc2-quartz-core` — the modern, maintained
/// successors to the deprecated `cocoa` and `objc` 0.2 crates.
#[cfg(target_os = "macos")]
pub fn apply_macos_window_styling(app: &tauri::App) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    apply_macos_window_styling_to(&window);
}

#[cfg(target_os = "macos")]
fn apply_macos_window_styling_to(window: &WebviewWindow) {
    window
        .with_webview(|webview| {
            use objc2_app_kit::{NSColor, NSWindow};

            unsafe {
                let ns_window: &NSWindow = &*webview.ns_window().cast();

                // Transparent window background so rounded corners don't
                // show an opaque rectangle behind the web content.
                ns_window.setOpaque(false);
                ns_window.setBackgroundColor(Some(&NSColor::clearColor()));
                // Keep the system drop-shadow so the window doesn't look flat.
                // This shadow is what gives macOS apps their subtle border
                // appearance (like Chrome).
                ns_window.setHasShadow(true);

                // Round the content view via its backing CALayer.
                let content_view = ns_window
                    .contentView()
                    .expect("NSWindow.contentView() should not be null");

                content_view.setWantsLayer(true);
                if let Some(layer) = content_view.layer().as_ref() {
                    layer.setCornerRadius(10.0);
                    layer.setMasksToBounds(true);
                }
            }
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(value: &str) -> Url {
        Url::parse(value).unwrap()
    }

    #[test]
    fn bundled_navigation_allows_only_the_configured_app_origin() {
        assert!(is_allowed_app_navigation(
            &url("tauri://localhost/index.html"),
            None,
            false
        ));
        assert!(is_allowed_app_navigation(
            &url("http://tauri.localhost/settings"),
            None,
            false
        ));
        assert!(is_allowed_app_navigation(
            &url("https://tauri.localhost/settings"),
            None,
            true
        ));

        assert!(!is_allowed_app_navigation(
            &url("https://tauri.localhost/settings"),
            None,
            false
        ));
        assert!(!is_allowed_app_navigation(
            &url("http://tauri.localhost.evil.example/"),
            None,
            false
        ));
        assert!(!is_allowed_app_navigation(
            &url("https://example.com/"),
            None,
            false
        ));
        assert!(!is_allowed_app_navigation(
            &url("file:///tmp/untrusted.html"),
            None,
            false
        ));
        assert!(!is_allowed_app_navigation(
            &url("data:text/html,untrusted"),
            None,
            false
        ));
    }

    #[test]
    fn development_navigation_requires_the_exact_vite_origin() {
        let dev_url = url("http://localhost:1420");

        assert!(is_allowed_app_navigation(
            &url("http://localhost:1420/editor?file=test#selection"),
            Some(&dev_url),
            false
        ));
        assert!(!is_allowed_app_navigation(
            &url("http://localhost:3000/"),
            Some(&dev_url),
            false
        ));
        assert!(!is_allowed_app_navigation(
            &url("https://localhost:1420/"),
            Some(&dev_url),
            false
        ));
        assert!(!is_allowed_app_navigation(
            &url("http://127.0.0.1:1420/"),
            Some(&dev_url),
            false
        ));
    }

    #[test]
    fn native_title_is_file_first_and_sanitized() {
        assert_eq!(
            format_window_title("notes.rs", false),
            "notes.rs — Grayslate"
        );
        assert_eq!(
            format_window_title("notes.rs", true),
            "● notes.rs — Grayslate"
        );
        assert_eq!(
            format_window_title("unsafe\nname.rs", false),
            "unsafename.rs — Grayslate"
        );
        assert_eq!(format_window_title("\n", false), "New Slate — Grayslate");
    }

    #[test]
    fn registry_prevents_two_windows_from_reserving_the_same_path() {
        let registry = WindowRegistry::default();
        let path = Path::new("/tmp/shared.txt");
        let first = registry.reserve("first", path).unwrap();
        assert_eq!(registry.reserve("second", path), Err("first".to_string()));
        registry.commit_open("first", &first, path).unwrap();
        assert_eq!(registry.owner_for_path(path).as_deref(), Some("first"));
        assert_eq!(registry.reserve("second", path), Err("first".to_string()));
        registry.release_active("first");
        assert!(registry.reserve("second", path).is_ok());
    }

    #[test]
    fn cancelled_reservation_does_not_replace_the_active_document() {
        let registry = WindowRegistry::default();
        let first_path = Path::new("/tmp/first.txt");
        let first = registry.reserve("main", first_path).unwrap();
        registry.commit_open("main", &first, first_path).unwrap();

        let second_path = Path::new("/tmp/second.txt");
        let second = registry.reserve("main", second_path).unwrap();
        registry.cancel_open("main", &second);

        assert_eq!(registry.owner_for_path(first_path).as_deref(), Some("main"));
        assert!(registry.owner_for_path(second_path).is_none());
    }

    #[test]
    fn primary_role_moves_to_the_most_recently_focused_survivor() {
        let registry = WindowRegistry::default();
        registry.register_window("main", None);
        registry.register_window("editor-a", Some("main"));
        registry.register_window("editor-b", Some("main"));
        registry.note_focused("editor-b");
        registry.note_focused("editor-a");

        let promotion = registry.cleanup_window("main").unwrap();
        assert_eq!(promotion.window_label, "editor-a");
        assert!(registry.is_primary("editor-a"));
        assert!(!registry.is_primary("editor-b"));
    }

    #[test]
    fn child_inherits_layout_and_only_primary_authorizes_persistence() {
        let registry = WindowRegistry::default();
        registry.register_window("main", None);
        registry.resolve_sidebar_layout("main", "20".into(), "false".into());
        assert!(registry.note_sidebar_setting("main", SETTING_SIDEBAR_WIDTH, Some("26")));
        assert!(registry.note_sidebar_setting("main", SETTING_SIDEBAR_OPEN, Some("true")));

        registry.register_window("editor-a", Some("main"));
        let inherited = registry.resolve_sidebar_layout("editor-a", "20".into(), "false".into());
        assert_eq!(inherited.width, "26");
        assert_eq!(inherited.open, "true");
        assert!(!registry.note_sidebar_setting("editor-a", SETTING_SIDEBAR_WIDTH, Some("24")));

        let promotion = registry.cleanup_window("main").unwrap();
        assert_eq!(promotion.window_label, "editor-a");
        assert_eq!(promotion.sidebar_layout.unwrap().width, "24");
        assert!(registry.note_sidebar_setting("editor-a", SETTING_SIDEBAR_OPEN, Some("false")));
    }
}
