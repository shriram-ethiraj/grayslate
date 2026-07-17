/// Every application command exposed through `tauri::generate_handler!`.
///
/// `build.rs` uses this list to generate a separate allow/deny permission for
/// each command. Keep it in sync with the invoke handler in `lib.rs`; a missing
/// entry intentionally makes that command unavailable to the webview.
pub const APP_COMMANDS: &[&str] = &[
    "autosave_activate_document",
    "autosave_activate_untitled",
    "autosave_flush_before_switch",
    "autosave_notify_changed",
    "autosave_set_csv_mode",
    "autosave_set_language_hint",
    "autosave_submit_content",
    "cancel_editor_find",
    "cancel_file_read",
    "cancel_markdown_preview",
    "cancel_sidebar_search",
    "cancel_transformation",
    "check_for_updates",
    "csv_cancel",
    "csv_dispose",
    "csv_flush_text",
    "csv_get_cell",
    "csv_get_rows",
    "csv_initialize",
    "csv_mutate",
    "csv_redo",
    "csv_undo",
    "delete_file",
    "detect_by_filename",
    "detect_language",
    "duplicate_file",
    "duplicate_local_file_as_slate",
    "editor_detect_indent",
    "editor_find_scan",
    "editor_find_selection",
    "execute_transformation",
    "get_all_settings",
    "get_app_info",
    "get_app_setting",
    "get_last_active_document",
    "get_memory_info",
    "get_recent_files",
    "install_available_update",
    "open_about_link",
    "open_markdown_link",
    "pick_document",
    "pick_notes_root",
    "pick_save_document",
    "prepare_close",
    "read_file_content",
    "read_markdown_preview_asset",
    "rename_file",
    "render_markdown_preview",
    "reset_notes_root",
    "resolve_default_notes_root",
    "resolve_notes_root",
    "reveal_document",
    "save_untitled_slate",
    "search_sidebar_files",
    "set_app_setting",
    "set_last_active_document",
    "set_menu_save_enabled",
    "set_menu_word_wrap",
    "suggest_name_for_file",
    "suggest_slate_name",
    "untrack_local_file",
    "write_file_content",
];

/// Commands compiled only into the end-to-end test build (`--features e2e`).
///
/// `build.rs` appends these to the generated permission manifest when the `e2e`
/// Cargo feature is active, so their ACL permissions exist for the test-only
/// runtime capability. They must never reach a release binary.
#[cfg_attr(not(feature = "e2e"), allow(dead_code))]
pub const E2E_COMMANDS: &[&str] = &["e2e_open_path", "e2e_save_path"];
