pub mod csv;
pub mod detection;
pub mod file;
pub mod findstats;
pub mod memory;
pub mod naming;
pub mod search;
pub mod transform;
pub mod update;

/// Tauri event emitted after any file operation that changes the recent-files
/// list (open, save, rename, delete, duplicate). The frontend sidebar listens
/// for this event and refreshes its list.
pub const RECENT_FILES_UPDATED_EVENT: &str = "files://recent-updated";
