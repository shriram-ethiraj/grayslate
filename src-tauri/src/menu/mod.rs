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
pub fn build_native_menu(
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
pub fn handle_macos_menu_event(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
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

/// Tauri command called from Svelte whenever `editorState.wordWrap` changes.
///
/// On macOS it updates the native `CheckMenuItem` so the system menu bar
/// checkmark stays in sync with the in-app context menu and keyboard shortcut.
/// On other platforms this is a no-op; the command is always registered so
/// the invoke handler list does not need conditional compilation.
#[tauri::command]
pub fn set_menu_word_wrap(app: tauri::AppHandle, checked: bool) {
    #[cfg(target_os = "macos")]
    {
        use tauri::Manager;
        if let Some(state) = app.try_state::<MacOsMenuState>() {
            if let Ok(item) = state.word_wrap_item.lock() {
                let _ = item.set_checked(checked);
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, checked);
}
