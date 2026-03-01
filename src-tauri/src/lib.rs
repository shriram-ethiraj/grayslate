
/// Maximum file size allowed to be opened: 200 MB.
const MAX_FILE_SIZE: u64 = 200 * 1024 * 1024;

/// Apply macOS-specific window styling: rounded corners + shadow.
///
/// `decorations: false` gives a borderless NSWindow (sharp rectangle).
/// We make the native window background transparent, then round the
/// content-view's CALayer so the web content clips to rounded corners
/// while preserving the window shadow.
#[cfg(target_os = "macos")]
#[allow(deprecated)] // cocoa crate is deprecated in favor of objc2-app-kit, but still functional
fn apply_macos_window_styling(app: &tauri::App) {
    use tauri::Manager;

    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    window
        .with_webview(|webview| {
            use cocoa::appkit::{NSColor, NSView, NSWindow};
            use cocoa::base::{id, nil, NO, YES};
            use objc::{msg_send, sel, sel_impl};

            unsafe {
                let ns_window: id = webview.ns_window() as id;

                // Transparent window background so rounded corners don't
                // show an opaque rectangle behind the web content.
                ns_window.setOpaque_(NO);
                ns_window.setBackgroundColor_(NSColor::clearColor(nil));
                // Keep the system drop-shadow so the window doesn't look flat.
                ns_window.setHasShadow_(YES);

                // Round the content view via its backing CALayer.
                let content_view: id = ns_window.contentView();
                content_view.setWantsLayer(YES);
                let layer: id = msg_send![content_view, layer];
                let _: () = msg_send![layer, setCornerRadius: 10.0_f64];
                let _: () = msg_send![layer, setMasksToBounds: YES];
            }
        })
        .ok();
}

/// Read a file from disk and return its text content.
///
/// Returns an error string (forwarded to the frontend) when:
/// - the path cannot be stat-ed or read, or
/// - the file exceeds the 50 MB limit.
#[tauri::command]
async fn read_file_content(path: String) -> Result<String, String> {
    let metadata = std::fs::metadata(&path).map_err(|e| format!("Cannot access file: {}", e))?;

    if metadata.len() > MAX_FILE_SIZE {
        let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
        return Err(format!(
            "File is too large ({:.1} MB). The maximum allowed size is 200 MB.",
            size_mb
        ));
    }

    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            apply_macos_window_styling(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![read_file_content])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
