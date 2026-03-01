use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::Emitter;

/// Maximum file size allowed to be opened: 200 MB.
const MAX_FILE_SIZE: u64 = 200 * 1024 * 1024;

/// Read a file from disk and return its text content.
///
/// Returns an error string (forwarded to the frontend) when:
/// - the path cannot be stat-ed or read, or
/// - the file exceeds the 50 MB limit.
#[tauri::command]
async fn read_file_content(path: String) -> Result<String, String> {
    let metadata = std::fs::metadata(&path).map_err(|e| {
        format!("Cannot access file: {}", e)
    })?;

    if metadata.len() > MAX_FILE_SIZE {
        let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
        return Err(format!(
            "File is too large ({:.1} MB). The maximum allowed size is 200 MB.",
            size_mb
        ));
    }

    std::fs::read_to_string(&path).map_err(|e| {
        format!("Failed to read file: {}", e)
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            // Build "File" submenu
            let open_item = MenuItem::with_id(
                app,
                "open-file",
                "Open File...",
                true,
                // Keyboard shortcut: Ctrl+O on Windows/Linux, Cmd+O on macOS
                Some("CmdOrCtrl+O"),
            )?;

            let file_menu = Submenu::with_items(app, "File", true, &[&open_item])?;

            // Build "Edit" submenu with predefined clipboard items.
            // On macOS these PredefinedMenuItems wire up the WKWebView responder
            // chain so that Cmd+Z/X/C/V/A are forwarded to the focused web content.
            let undo_item = PredefinedMenuItem::undo(app, Some("Undo"))?;
            let redo_item = PredefinedMenuItem::redo(app, Some("Redo"))?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let cut_item = PredefinedMenuItem::cut(app, Some("Cut"))?;
            let copy_item = PredefinedMenuItem::copy(app, Some("Copy"))?;
            let paste_item = PredefinedMenuItem::paste(app, Some("Paste"))?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let select_all_item = PredefinedMenuItem::select_all(app, Some("Select All"))?;

            let edit_menu = Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &undo_item,
                    &redo_item,
                    &sep1,
                    &cut_item,
                    &copy_item,
                    &paste_item,
                    &sep2,
                    &select_all_item,
                ],
            )?;

            let menu = Menu::with_items(app, &[&file_menu, &edit_menu])?;
            app.set_menu(menu)?;

            Ok(())
        })
        // Emit a frontend event when the "Open File..." menu item is clicked.
        // The frontend then shows the native file picker and calls read_file_content.
        .on_menu_event(|app, event| {
            if event.id() == "open-file" {
                app.emit("menu://open-file", ()).unwrap_or_default();
            }
        })
        .invoke_handler(tauri::generate_handler![read_file_content])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
