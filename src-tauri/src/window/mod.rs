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
