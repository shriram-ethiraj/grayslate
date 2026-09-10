use tauri::Manager;

pub mod autosave;
pub mod character_encoding;
pub mod commands;
pub mod csv;
pub mod detection;
pub mod document;
pub mod filesystem;
pub mod findstats;
pub mod line_ending;
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
#[cfg(test)]
mod file_association_tests;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    // This must be the first plugin so a second OS "Open With" activation is
    // redirected before any other plugin can observe or mutate app state.
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
        commands::external_open::focus_main_window(app);
        commands::external_open::enqueue_cli_activation(
            app,
            args.into_iter().skip(1),
            std::path::Path::new(&cwd),
        );
    }));

    let builder = builder
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
    let builder = builder.plugin(
        tauri_plugin_window_state::Builder::default()
            // Restore geometry only. Visibility is controlled explicitly by
            // the window builder so no renderer bootstrap can keep a native
            // window hidden.
            .with_state_flags(
                tauri_plugin_window_state::StateFlags::SIZE
                    | tauri_plugin_window_state::StateFlags::POSITION
                    | tauri_plugin_window_state::StateFlags::MAXIMIZED
                    | tauri_plugin_window_state::StateFlags::FULLSCREEN,
            )
            .with_filter(|label| label == "main")
            .build(),
    );

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

    // File paths delivered by a native drop must enter the document grant
    // boundary in Rust. The webview observes the same Tauri event only to
    // render drop feedback; it cannot authorize paths itself.
    let builder = builder.on_window_event(|window, event| match event {
        tauri::WindowEvent::Focused(true) => {
            if let Some(registry) = window.app_handle().try_state::<window::WindowRegistry>() {
                registry.note_focused(window.label());
            }
            #[cfg(target_os = "macos")]
            menu::sync_native_menu_for_window(window.app_handle(), window.label());
        }
        tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
            commands::external_open::enqueue_dropped_paths(
                window.app_handle(),
                window.label(),
                paths.clone(),
            );
        }
        tauri::WindowEvent::Destroyed => {
            if let Some(registry) = window.app_handle().try_state::<window::WindowRegistry>() {
                if let Some(promotion) = registry.cleanup_window(window.label()) {
                    if let (Some(storage), Some(layout)) = (
                        window.app_handle().try_state::<storage::AppStorage>(),
                        promotion.sidebar_layout,
                    ) {
                        if let Err(error) = storage
                            .set_setting(storage::SETTING_SIDEBAR_WIDTH, Some(&layout.width))
                            .and_then(|_| {
                                storage
                                    .set_setting(storage::SETTING_SIDEBAR_OPEN, Some(&layout.open))
                            })
                        {
                            eprintln!(
                                "Failed to persist promoted window {} layout: {error}",
                                promotion.window_label
                            );
                        }
                    }
                }
            }
            if let Some(registry) = window
                .app_handle()
                .try_state::<autosave::AutosaveRegistry>()
            {
                registry.unregister(window.label());
            }
            if let Some(state) = window
                .app_handle()
                .try_state::<commands::search::SearchRuntimeState>()
            {
                state.cleanup_window(window.label());
            }
            if let Some(state) = window
                .app_handle()
                .try_state::<commands::transform::TransformationCancellationRegistry>(
            ) {
                state.cleanup_window(window.label());
            }
            if let Some(registry) = window
                .app_handle()
                .try_state::<document::DocumentRegistry>()
            {
                registry.revoke_window(window.label());
            }
            if let Some(state) = window
                .app_handle()
                .try_state::<commands::external_open::ExternalOpenState>()
            {
                state.cleanup_window(window.label());
            }
            if let Some(state) = window
                .app_handle()
                .try_state::<commands::file::FileReadCancellationRegistry>()
            {
                state.cancel_window_request(window.label());
            }
            if let Some(state) = window
                .app_handle()
                .try_state::<commands::csv::CsvSessionRegistry>()
            {
                state.dispose_window(window.label());
            }
            if let Some(state) = window
                .app_handle()
                .try_state::<commands::findstats::EditorFindState>()
            {
                state.cleanup_window(window.label());
            }
            if let Some(state) = window
                .app_handle()
                .try_state::<commands::markdown::MarkdownPreviewState>()
            {
                state.cleanup_window(window.label());
            }
            if let Some(state) = window
                .app_handle()
                .try_state::<commands::clipboard::ClipboardCopyRegistry>()
            {
                state.cleanup_window(window.label());
            }
        }
        _ => {}
    });

    builder
        .setup(|app| {
            let storage = storage::AppStorage::initialize(app.handle()).map_err(|error| {
                std::io::Error::other(format!("Failed to initialize app storage: {}", error))
            })?;
            app.manage(storage);
            app.manage(commands::file::FileReadCancellationRegistry::default());
            app.manage(document::DocumentRegistry::default());
            app.manage(commands::external_open::ExternalOpenState::default());
            app.manage(commands::search::SearchRuntimeState::default());
            app.manage(commands::transform::TransformationCancellationRegistry::default());
            app.manage(commands::findstats::EditorFindState::default());
            app.manage(commands::markdown::MarkdownPreviewState::default());
            app.manage(commands::csv::CsvSessionRegistry::default());
            app.manage(commands::clipboard::ClipboardCopyRegistry::default());
            app.manage(autosave::AutosaveRegistry::default());
            app.manage(save_coordinator::SaveCoordinator::default());
            app.manage(commands::update::UpdateOperationState::default());
            app.manage(commands::update::AutomaticUpdateScheduler::default());
            app.manage(window::WindowRegistry::default());

            // Spawn the background autosave timer thread.
            let timer_handle = app.handle().clone();
            std::thread::spawn(move || autosave::run_timer_loop(timer_handle));

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            commands::update::start_automatic_update_scheduler(app.handle().clone());

            window::create_main_window(app)?;

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            commands::external_open::flush_staged_path_activations(app.handle());

            #[cfg(any(target_os = "windows", target_os = "linux"))]
            commands::external_open::enqueue_initial_activation(app.handle());

            // Test-only: grant the e2e fixture open/save shims at runtime. This
            // capability lives outside the auto-scanned `capabilities/` dir and
            // is compiled + added only under `--features e2e`, so a release
            // build never references it or its permissions.
            #[cfg(feature = "e2e")]
            app.add_capability(include_str!("../e2e-capabilities/e2e.json"))?;
            // Test-only: holds pre-selected native-dialog answers so a spec can
            // click the real Open / Save As menu items.
            #[cfg(feature = "e2e")]
            app.manage(commands::e2e::QueuedDialogPaths::default());
            #[cfg(feature = "e2e")]
            app.manage(commands::e2e::ExternalActionProbe::default());

            Ok(())
        })
        // NOTE: closing is driven entirely from the frontend's
        // `onCloseRequested` handler, which flushes and destroys the window via
        // the `prepare_close` command. A Rust `CloseRequested` hook cannot do
        // it: Tauri auto-prevents every close while a JS listener is registered
        // for that event, so `Window::close` from Rust never terminates.
        .invoke_handler(tauri::generate_handler![
            commands::file::cancel_file_read,
            window::cancel_document_open,
            window::claim_document_open,
            window::create_editor_window,
            commands::file::delete_file,
            commands::file::duplicate_file,
            commands::file::untrack_local_file,
            commands::file::duplicate_local_file_as_slate,
            commands::file::get_all_settings,
            commands::file::get_app_setting,
            commands::update::get_update_status,
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
            commands::update::respond_update_install_preflight,
            commands::external::get_app_info,
            commands::external::open_about_link,
            commands::external::open_markdown_link,
            commands::external_open::take_external_open_request,
            commands::autosave::autosave_activate_untitled,
            commands::autosave::autosave_activate_document,
            commands::autosave::autosave_notify_changed,
            commands::autosave::autosave_submit_content,
            commands::autosave::autosave_flush_before_switch,
            commands::autosave::autosave_set_csv_mode,
            commands::autosave::autosave_set_eol,
            commands::autosave::autosave_set_language_hint,
            commands::autosave::prepare_close,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_open_path,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_arm_minimize_probe,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_arm_navigation_probe,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_arm_operation_gate,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_drop_paths,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_force_autosave_cycle,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_focus_window,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_minimize_observation,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_navigation_observation,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_operation_gate_reached,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_queue_external_confirmation,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_release_operation_gate,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_save_path,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_queue_open_path,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_queue_save_path,
            #[cfg(feature = "e2e")]
            commands::e2e::e2e_take_external_action,
            menu::set_menu_word_wrap,
            menu::set_menu_save_enabled,
            window::sync_window_title,
            window::take_window_launch_intent,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            match event {
                tauri::RunEvent::Opened { urls } => {
                    if let Err(error) = window::reopen_or_create_main_window(app) {
                        eprintln!("Failed to open a Grayslate window: {error}");
                    }
                    commands::external_open::enqueue_opened_urls(app, urls);
                }
                tauri::RunEvent::Reopen { .. } => {
                    if let Err(error) = window::reopen_or_create_main_window(app) {
                        eprintln!("Failed to reopen Grayslate: {error}");
                    }
                }
                _ => {}
            }

            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
