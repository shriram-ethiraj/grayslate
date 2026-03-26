/// Holds macOS-specific menu item handles so the event handler can mutate
/// them without going through `menu.get()`, which does NOT recurse into submenus.
#[cfg(target_os = "macos")]
struct MacOsMenuState {
    word_wrap_item: std::sync::Mutex<tauri::menu::CheckMenuItem<tauri::Wry>>,
    save_file_item: std::sync::Mutex<tauri::menu::MenuItem<tauri::Wry>>,
}

/// Build the macOS-native menu bar (File + Edit + View).
///
/// On macOS the in-window shadcn Menubar is hidden; this native menu
/// provides the same actions via the system menu bar at the top of the
/// screen.  Menu events are forwarded to the webview as Tauri events so
/// the existing Svelte action handlers can process them unchanged.
#[cfg(target_os = "macos")]
pub fn build_native_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
    use tauri::Manager;

    let app_menu = SubmenuBuilder::new(app, "Grayslate")
        .item(&MenuItemBuilder::with_id("check-for-updates", "Check for Updates...").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("about", "About Grayslate").build(app)?)
        .build()?;

    let save_file_item = MenuItemBuilder::with_id("save-file", "Save")
        .accelerator("CmdOrCtrl+S")
        .build(app)?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(
            &MenuItemBuilder::with_id("new-file", "New Slate")
                .accelerator("CmdOrCtrl+N")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("open-file", "Open File...")
                .accelerator("CmdOrCtrl+O")
                .build(app)?,
        )
        .separator()
        .item(&save_file_item)
        .item(
            &MenuItemBuilder::with_id("save-file-as", "Save As...")
                .accelerator("CmdOrCtrl+Shift+S")
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
        save_file_item: std::sync::Mutex::new(save_file_item),
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
        .item(
            &MenuItemBuilder::with_id("edit-go-to-line", "Go To Line...")
                .accelerator("CmdOrCtrl+G")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("edit-find", "Find...")
                .accelerator("CmdOrCtrl+F")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("edit-find-files", "Find Files...")
                .accelerator("CmdOrCtrl+P")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("edit-replace", "Replace...")
                .accelerator("CmdOrCtrl+Alt+F")
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

    let view_menu = SubmenuBuilder::new(app, "View")
        .item(
            &MenuItemBuilder::with_id("view-increase-font-size", "Increase Font Size")
                .accelerator("CmdOrCtrl+=")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("view-decrease-font-size", "Decrease Font Size")
                .accelerator("CmdOrCtrl+-")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("view-reset-font-size", "Reset Font Size")
                .accelerator("CmdOrCtrl+0")
                .build(app)?,
        )
        .build()?;

    let menu_builder = MenuBuilder::new(app)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu);

    menu_builder.build()
}

/// Handle macOS native menu events: forward each item click to the webview
/// as a Tauri event so the Svelte action handlers can process them unchanged.
///
/// - `menu://new-file`     → EditorWrapper's createNewFile()
/// - `menu://open-file`    → EditorWrapper's openFile()
/// - `menu://edit-action`  → Titlebar's handleEdit(action) / word-wrap toggle
/// - `menu://view-action`  → Titlebar's handleView(action)
#[cfg(target_os = "macos")]
pub fn handle_macos_menu_event(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    use tauri::{Emitter, Manager};

    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    match event.id.as_ref() {
        "about" => {
            let _ = window.emit("menu://about", true);
        }
        "check-for-updates" => {
            let _ = window.emit("menu://check-for-updates", true);
        }
        "new-file" => {
            let _ = window.emit("menu://new-file", true);
        }
        "open-file" => {
            let _ = window.emit("menu://open-file", true);
        }
        "save-file" => {
            let _ = window.emit("menu://save-file", true);
        }
        "save-file-as" => {
            let _ = window.emit("menu://save-file-as", true);
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
        "edit-go-to-line" => {
            let _ = window.emit("menu://edit-action", "goToLine");
        }
        "edit-find" => {
            let _ = window.emit("menu://edit-action", "find");
        }
        "edit-find-files" => {
            let _ = window.emit("menu://edit-action", "findFiles");
        }
        "edit-replace" => {
            let _ = window.emit("menu://edit-action", "replace");
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
        "view-increase-font-size" => {
            let _ = window.emit("menu://view-action", "increaseFontSize");
        }
        "view-decrease-font-size" => {
            let _ = window.emit("menu://view-action", "decreaseFontSize");
        }
        "view-reset-font-size" => {
            let _ = window.emit("menu://view-action", "resetFontSize");
        }
        _ => {}
    }
}

/// Tauri command called from Svelte whenever `editorState.isDirty` changes.
///
/// On macOS it enables or disables the native "Save" menu item to match the
/// editor's dirty state. On other platforms this is a no-op.
#[tauri::command]
pub fn set_menu_save_enabled(app: tauri::AppHandle, enabled: bool) {
    #[cfg(target_os = "macos")]
    {
        use tauri::Manager;
        if let Some(state) = app.try_state::<MacOsMenuState>() {
            if let Ok(item) = state.save_file_item.lock() {
                let _ = item.set_enabled(enabled);
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, enabled);
}
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
