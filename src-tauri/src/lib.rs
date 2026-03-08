pub mod commands;
pub mod menu;
pub mod window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init());

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_window_state::Builder::default().build());

    // Attach the native macOS menu bar and its event handler only on macOS.
    // On Windows/Linux the existing in-window shadcn Menubar is used instead.
    #[cfg(target_os = "macos")]
    let builder = builder
        .menu(menu::build_native_menu)
        .on_menu_event(menu::handle_macos_menu_event);

    builder
        .setup(|app| {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            #[cfg(target_os = "macos")]
            window::apply_macos_window_styling(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::file::read_file_content,
            commands::file::resolve_default_notes_root,
            commands::file::write_file_content,
            commands::memory::get_memory_info,
            commands::update::check_for_updates,
            commands::update::install_available_update,
            menu::set_menu_word_wrap,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
