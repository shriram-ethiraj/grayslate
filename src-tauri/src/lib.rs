
/// Maximum file size allowed to be opened: 200 MB.
const MAX_FILE_SIZE: u64 = 200 * 1024 * 1024;

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
        .invoke_handler(tauri::generate_handler![read_file_content])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
