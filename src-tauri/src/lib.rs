use tauri::Manager;

pub mod autosave;
pub mod commands;
pub mod csv;
pub mod detection;
pub mod filesystem;
pub mod findstats;
pub mod markdown_preview;
pub mod menu;
pub mod naming;
pub mod search;
pub mod storage;
pub mod window;

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
            app.manage(commands::search::SearchRuntimeState::default());
            app.manage(commands::transform::TransformationCancellationRegistry::default());
            app.manage(commands::findstats::EditorFindState::default());
            app.manage(commands::markdown::MarkdownPreviewState::default());
            app.manage(commands::csv::CsvSessionRegistry::default());
            app.manage(autosave::AutosaveRegistry::default());

            // Spawn the background autosave timer thread.
            let timer_handle = app.handle().clone();
            std::thread::spawn(move || autosave::run_timer_loop(timer_handle));

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            #[cfg(target_os = "macos")]
            window::apply_macos_window_styling(app);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let registry = window.app_handle().state::<autosave::AutosaveRegistry>();
                let label = window.label().to_string();

                if registry.has_unsaved_changes(&label) {
                    api.prevent_close();
                    let window_handle = window.clone();
                    tauri::async_runtime::spawn(async move {
                        flush_on_close(&window_handle).await;
                        let _ = window_handle.close();
                    });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::file::cancel_file_read,
            commands::file::delete_file,
            commands::file::duplicate_file,
            commands::file::untrack_local_file,
            commands::file::duplicate_local_file_as_slate,
            commands::file::get_all_settings,
            commands::file::get_app_setting,
            commands::file::get_recent_files,
            commands::file::prepare_file_open,
            commands::file::read_file_content,
            commands::file::rename_file,
            commands::file::resolve_notes_root,
            commands::file::resolve_default_notes_root,
            commands::file::set_app_setting,
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
            commands::csv::csv_initialize,
            commands::csv::csv_dispose,
            commands::csv::csv_get_rows,
            commands::csv::csv_get_cell,
            commands::csv::csv_mutate,
            commands::csv::csv_undo,
            commands::csv::csv_redo,
            commands::csv::csv_flush_text,
            commands::csv::csv_cancel,
            commands::search::cancel_sidebar_search,
            commands::search::search_sidebar_files,
            commands::transform::cancel_transformation,
            commands::transform::execute_transformation,
            commands::update::check_for_updates,
            commands::update::install_available_update,
            commands::autosave::autosave_register,
            commands::autosave::autosave_notify_changed,
            commands::autosave::autosave_submit_content,
            commands::autosave::autosave_flush_before_switch,
            commands::autosave::autosave_set_csv_mode,
            commands::autosave::classify_source,
            menu::set_menu_word_wrap,
            menu::set_menu_save_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ---------------------------------------------------------------------------
// Close-request flush logic
// ---------------------------------------------------------------------------

/// Flush pending autosave changes before the window closes.
///
/// For CSV table mode, serializes directly from `CsvSession`.
/// For text mode, emits a flush event to the frontend and waits for the
/// FE to respond by calling `autosave_submit_content`.  If the FE doesn't
/// respond within 3 seconds, allow the close anyway.
async fn flush_on_close(window: &tauri::Window) {
    use tauri::Emitter;

    let label = window.label().to_string();
    let app = window.app_handle();
    let registry = app.state::<autosave::AutosaveRegistry>();

    let doc_info = match registry.get_document_info(&label) {
        Some(info) => info,
        None => return,
    };

    if !doc_info.is_dirty || !matches!(doc_info.source, storage::FileSource::Slates) {
        return;
    }

    if doc_info.csv_table_active {
        // CSV: serialize directly from CsvSession and write
        let csv_registry = app.state::<commands::csv::CsvSessionRegistry>();
        if let Some((_, content)) = csv_registry.try_flush_for_autosave(&label) {
            if let Some(path) = &doc_info.path {
                let path = path.clone();
                let _ = tauri::async_runtime::spawn_blocking(move || {
                    autosave::autosave_write_to_disk(&path, &content)
                })
                .await;
            }
        }
    } else {
        // Text: ask FE for content, wait with timeout
        let _ = window.emit(
            autosave::AUTOSAVE_FLUSH_BEFORE_CLOSE_EVENT,
            autosave::ContentRequestPayload { request_id: 0 },
        );

        // Wait up to 3 seconds for the FE to call autosave_submit_content.
        // The submit_content command will complete the save; we just need to
        // wait long enough for it to finish.
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(3);
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if !registry.has_unsaved_changes(&label) {
                break;
            }
            if start.elapsed() >= timeout {
                eprintln!(
                    "Autosave: close-flush timed out for window '{}'; accepting potential data loss.",
                    label
                );
                break;
            }
        }
    }
}
