use tauri::Manager;

pub mod autosave;
pub mod commands;
pub mod csv;
pub mod detection;
pub mod document;
pub mod filesystem;
pub mod findstats;
pub mod markdown_preview;
pub mod menu;
pub mod naming;
pub mod save_coordinator;
pub mod search;
pub mod storage;
pub mod update_policy;
pub mod window;

#[cfg(test)]
mod capability_tests;
#[cfg(test)]
mod command_names;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        // Block the webview-native Find UI so Cmd/Ctrl+F always stays inside the app.
        // Also block the default browser context menu in production builds.
        .plugin(
            tauri_plugin_prevent_default::Builder::new()
                .with_flags(if cfg!(not(debug_assertions)) {
                    tauri_plugin_prevent_default::Flags::FIND
                        | tauri_plugin_prevent_default::Flags::CONTEXT_MENU
                } else {
                    tauri_plugin_prevent_default::Flags::FIND
                })
                .build(),
        );

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_window_state::Builder::default().build());

    // Test-only WebdriverIO bridge. The dependency, plugin commands, and ACL
    // grants are absent unless the dedicated E2E feature is enabled.
    #[cfg(feature = "e2e")]
    let builder = builder.plugin(tauri_plugin_wdio::init());

    // Attach the native macOS menu bar and its event handler only on macOS.
    // On Windows/Linux the existing in-window shadcn Menubar is used instead.
    #[cfg(target_os = "macos")]
    let builder = builder
        .menu(menu::build_native_menu)
        .on_menu_event(menu::handle_macos_menu_event);

    builder
        .setup(|app| {
            let storage = storage::AppStorage::initialize(app.handle()).map_err(|error| {
                std::io::Error::other(format!("Failed to initialize app storage: {}", error))
            })?;
            app.manage(storage);
            app.manage(commands::file::FileReadCancellationRegistry::default());
            app.manage(document::DocumentRegistry::default());
            app.manage(commands::search::SearchRuntimeState::default());
            app.manage(commands::transform::TransformationCancellationRegistry::default());
            app.manage(commands::findstats::EditorFindState::default());
            app.manage(commands::markdown::MarkdownPreviewState::default());
            app.manage(commands::csv::CsvSessionRegistry::default());
            app.manage(commands::clipboard::ClipboardCopyRegistry::default());
            app.manage(autosave::AutosaveRegistry::default());
            app.manage(save_coordinator::SaveCoordinator::default());
            app.manage(commands::update::UpdateOperationState::default());

            // Spawn the background autosave timer thread.
            let timer_handle = app.handle().clone();
            std::thread::spawn(move || autosave::run_timer_loop(timer_handle));

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            window::create_main_window(app)?;

            #[cfg(target_os = "macos")]
            window::apply_macos_window_styling(app);

            // Test-only: grant the e2e fixture open/save shims at runtime. This
            // capability lives outside the auto-scanned `capabilities/` dir and
            // is compiled + added only under `--features e2e`, so a release
            // build never references it or its permissions.
            #[cfg(feature = "e2e")]
            app.add_capability(include_str!("../e2e-capabilities/e2e.json"))?;

            Ok(())
        })
        // NOTE: closing is driven entirely from the frontend's
        // `onCloseRequested` handler, which flushes and destroys the window via
        // the `prepare_close` command. A Rust `CloseRequested` hook cannot do
        // it: Tauri auto-prevents every close while a JS listener is registered
        // for that event, so `Window::close` from Rust never terminates.
        .invoke_handler(tauri::generate_handler![
            commands::file::cancel_file_read,
            commands::file::delete_file,
            commands::file::duplicate_file,
            commands::file::untrack_local_file,
            commands::file::duplicate_local_file_as_slate,
            commands::file::get_all_settings,
            commands::file::get_app_setting,
            commands::file::get_last_active_document,
            commands::file::get_recent_files,
            commands::file::pick_document,
            commands::file::pick_notes_root,
            commands::file::pick_save_document,
            commands::file::read_file_content,
            commands::file::reveal_document,
            commands::file::rename_file,
            commands::file::resolve_notes_root,
            commands::file::reset_notes_root,
            commands::file::resolve_default_notes_root,
            commands::file::set_app_setting,
            commands::file::set_last_active_document,
            commands::file::write_file_content,
            commands::memory::get_memory_info,
            commands::detection::detect_language,
            commands::detection::detect_by_filename,
            commands::naming::save_untitled_slate,
            commands::naming::suggest_slate_name,
            commands::naming::suggest_name_for_file,
            commands::findstats::editor_find_scan,
            commands::findstats::editor_find_selection,
            commands::findstats::cancel_editor_find,
            commands::markdown::render_markdown_preview,
            commands::markdown::cancel_markdown_preview,
            commands::markdown::read_markdown_preview_asset,
            commands::csv::csv_initialize,
            commands::csv::csv_dispose,
            commands::csv::csv_get_rows,
            commands::csv::csv_get_cell,
            commands::csv::csv_mutate,
            commands::csv::csv_undo,
            commands::csv::csv_redo,
            commands::csv::csv_flush_text,
            commands::csv::csv_copy_to_clipboard,
            commands::csv::csv_cancel,
            commands::clipboard::clipboard_write_chunk,
            commands::search::cancel_sidebar_search,
            commands::search::search_sidebar_files,
            commands::transform::cancel_transformation,
            commands::transform::execute_transformation,
            commands::transform::editor_detect_indent,
            commands::update::check_for_updates,
            commands::update::install_available_update,
            commands::external::get_app_info,
            commands::external::open_about_link,
            commands::external::open_markdown_link,
            commands::autosave::autosave_activate_untitled,
            commands::autosave::autosave_activate_document,
            commands::autosave::autosave_notify_changed,
            commands::autosave::autosave_submit_content,
            commands::autosave::autosave_flush_before_switch,
            commands::autosave::autosave_set_csv_mode,
            commands::autosave::autosave_set_language_hint,
            commands::autosave::prepare_close,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_open_path,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_save_path,
            menu::set_menu_word_wrap,
            menu::set_menu_save_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
