
/// Maximum file size allowed to be opened: 200 MB.
const MAX_FILE_SIZE: u64 = 200 * 1024 * 1024;

/// Holds macOS-specific menu item handles so the event handler can mutate
/// them without going through `menu.get()`, which does NOT recurse into submenus.
#[cfg(target_os = "macos")]
struct MacOsMenuState {
    word_wrap_item: std::sync::Mutex<tauri::menu::CheckMenuItem<tauri::Wry>>,
}

/// Build the macOS-native menu bar (File + Edit).
///
/// On macOS the in-window shadcn Menubar is hidden; this native menu
/// provides the same actions via the system menu bar at the top of the
/// screen.  Menu events are forwarded to the webview as Tauri events so
/// the existing Svelte action handlers can process them unchanged.
#[cfg(target_os = "macos")]
fn build_native_menu(
    app: &tauri::AppHandle,
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
    use tauri::Manager;

    let app_menu = SubmenuBuilder::new(app, "Grayslate")
        .item(
            &MenuItemBuilder::with_id("about", "About Grayslate")
                .build(app)?,
        )
        .build()?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(
            &MenuItemBuilder::with_id("open-file", "Open File...")
                .accelerator("CmdOrCtrl+O")
                .build(app)?,
        )
        .build()?;

    // Word Wrap is a checkbox whose default (unchecked) mirrors editorState.wordWrap = false.
    // We store a reference in managed state because menu.get() does NOT recurse into submenus.
    let word_wrap_item = CheckMenuItemBuilder::with_id("edit-word-wrap", "Word Wrap")
        .accelerator("Alt+Z")
        .checked(false)
        .build(app)?;

    // Store the CheckMenuItem handle in managed state so the event handler can
    // toggle it directly without going through the non-recursive menu.get().
    app.manage(MacOsMenuState {
        word_wrap_item: std::sync::Mutex::new(word_wrap_item.clone()),
    });

    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .item(
            &MenuItemBuilder::with_id("edit-undo", "Undo")
                .accelerator("CmdOrCtrl+Z")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("edit-redo", "Redo")
                .accelerator("CmdOrCtrl+Shift+Z")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("edit-cut", "Cut")
                .accelerator("CmdOrCtrl+X")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("edit-copy", "Copy")
                .accelerator("CmdOrCtrl+C")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("edit-paste", "Paste")
                .accelerator("CmdOrCtrl+V")
                .build(app)?,
        )
        .separator()
        .item(&word_wrap_item)
        .separator()
        .item(
            &MenuItemBuilder::with_id("edit-select-all", "Select All")
                .accelerator("CmdOrCtrl+A")
                .build(app)?,
        )
        .build()?;

    MenuBuilder::new(app)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .build()
}

/// Handle macOS native menu events: forward each item click to the webview
/// as a Tauri event so the Svelte action handlers can process them unchanged.
///
/// - `menu://open-file`    → EditorWrapper's openFile()
/// - `menu://edit-action`  → Titlebar's handleEdit(action) / word-wrap toggle
#[cfg(target_os = "macos")]
fn handle_macos_menu_event(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    use tauri::{Emitter, Manager};

    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    match event.id.as_ref() {
        "open-file" => {
            let _ = window.emit("menu://open-file", true);
        }
        "edit-undo" => {
            let _ = window.emit("menu://edit-action", "undo");
        }
        "edit-redo" => {
            let _ = window.emit("menu://edit-action", "redo");
        }
        "edit-cut" => {
            let _ = window.emit("menu://edit-action", "cut");
        }
        "edit-copy" => {
            let _ = window.emit("menu://edit-action", "copy");
        }
        "edit-paste" => {
            let _ = window.emit("menu://edit-action", "paste");
        }
        "edit-select-all" => {
            let _ = window.emit("menu://edit-action", "selectAll");
        }
        "edit-word-wrap" => {
            // macOS native menus automatically toggle `CheckMenuItem` state internally.
            // We retrieve the item from our managed state (since `menu.get()` does
            // not recurse into submenus) to read the new state and emit it to Svelte.
            let mut checked = false;
            if let Some(state) = app.try_state::<MacOsMenuState>() {
                if let Ok(ci) = state.word_wrap_item.lock() {
                    checked = ci.is_checked().unwrap_or(false);
                }
            }
            let _ = window.emit("menu://word-wrap-state", checked);
        }
        _ => {}
    }
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
fn apply_macos_window_styling(app: &tauri::App) {
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
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init());

    // Attach the native macOS menu bar and its event handler only on macOS.
    // On Windows/Linux the existing in-window shadcn Menubar is used instead.
    #[cfg(target_os = "macos")]
    let builder = builder
        .menu(build_native_menu)
        .on_menu_event(handle_macos_menu_event);

    builder
        .setup(|_app| {
            #[cfg(target_os = "macos")]
            apply_macos_window_styling(_app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![read_file_content])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
