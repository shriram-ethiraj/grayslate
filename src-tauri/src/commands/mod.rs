pub mod autosave;
pub mod csv;
pub mod detection;
#[cfg(feature = "e2e")]
pub mod e2e;
pub mod external;
pub mod file;
pub mod findstats;
pub mod markdown;
pub mod memory;
pub mod naming;
pub mod search;
pub mod transform;
pub mod update;

/// Tauri event emitted after any file operation that changes the recent-files
/// list (open, save, rename, delete, duplicate). The frontend sidebar listens
/// for this event and refreshes its list.
pub const RECENT_FILES_UPDATED_EVENT: &str = "files://recent-updated";
