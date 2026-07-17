use std::error::Error;
use tauri::http::HeaderValue;
use url::Url;

const PERMISSIONS_POLICY: &str =
    "camera=(), microphone=(), geolocation=(), display-capture=(), usb=(), serial=(), hid=(), payment=()";

/// Create the configured main window with fail-closed navigation hooks.
///
/// Tauri can only attach `on_new_window` while a webview is being built, so
/// `tauri.conf.json` sets `create: false` and this function recreates that same
/// configured window during setup. External links must continue to use the
/// validated Rust opener commands; the application webview itself never
/// navigates away and never creates child webviews.
pub fn create_main_window(app: &tauri::App) -> Result<(), Box<dyn Error>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == "main")
        .ok_or_else(|| std::io::Error::other("main window configuration is missing"))?;

    let dev_url = if cfg!(debug_assertions) {
        app.config().build.dev_url.clone()
    } else {
        None
    };
    let use_https_scheme = config.use_https_scheme;

    tauri::WebviewWindowBuilder::from_config(app.handle(), config)?
        .on_navigation(move |url| {
            is_allowed_app_navigation(url, dev_url.as_ref(), use_https_scheme)
        })
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        // Tauri 2.11.5's typed `Permissions-Policy` configuration currently
        // emits the misspelled header name `Permission-Policy`. Inject the
        // correctly named header on bundled custom-protocol responses until
        // the upstream implementation is corrected.
        .on_web_resource_request(|_, response| {
            response.headers_mut().insert(
                "Permissions-Policy",
                HeaderValue::from_static(PERMISSIONS_POLICY),
            );
        })
        .build()?;

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
    use tauri::Manager;

    let Some(window) = app.get_webview_window("main") else {
        return;
    };

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
}
