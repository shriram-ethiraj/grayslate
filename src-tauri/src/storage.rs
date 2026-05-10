use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::{path::BaseDirectory, AppHandle, Manager};

pub const SETTING_NOTES_ROOT: &str = "notes_root";
pub const SETTING_THEME: &str = "theme";
pub const SETTING_FONT_SIZE: &str = "font_size";
pub const SETTING_WORD_WRAP: &str = "word_wrap";
pub const SETTING_SIDEBAR_WIDTH: &str = "sidebar_width";
pub const SETTING_SIDEBAR_OPEN: &str = "sidebar_open";

/// All app setting keys that the app validates/converts at the command layer.
/// Used for batch-loading at startup so the frontend doesn't need to hardcode them.
pub const ALL_SETTING_KEYS: &[&str] = &[
    SETTING_NOTES_ROOT,
    SETTING_THEME,
    SETTING_FONT_SIZE,
    SETTING_WORD_WRAP,
    SETTING_SIDEBAR_WIDTH,
    SETTING_SIDEBAR_OPEN,
];

const DATABASE_FILENAME: &str = "grayslate.sqlite3";

#[derive(Clone)]
pub struct AppStorage {
    db_path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileSource {
    Slates,
    Local,
}

impl FileSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Slates => "slates",
            Self::Local => "local",
        }
    }
}

#[derive(Clone, Serialize)]
pub struct RecentFileRecord {
    pub path: String,
    pub file_name: String,
    pub extension: Option<String>,
    pub language: String,
    pub source: String,
    pub size_bytes: Option<u64>,
    pub file_modified_app_at: Option<i64>,
    pub file_modified_disk_at: Option<i64>,
    pub updated_at: i64,
}

struct FileMetadataSnapshot {
    path: String,
    path_key: String,
    file_name: String,
    extension: Option<String>,
    size_bytes: Option<u64>,
    file_modified_disk_at: Option<i64>,
}

impl AppStorage {
    pub fn initialize(app: &AppHandle) -> Result<Self, String> {
        let app_data_dir = app
            .path()
            .resolve("", BaseDirectory::AppData)
            .map_err(|error| format!("Unable to resolve app data directory: {}", error))?;

        fs::create_dir_all(&app_data_dir)
            .map_err(|error| format!("Failed to create app data directory: {}", error))?;

        let storage = Self {
            db_path: app_data_dir.join(DATABASE_FILENAME),
        };

        storage.run_migrations()?;
        Ok(storage)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        let connection = self.open_connection()?;
        connection
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("Failed to read app setting: {}", error))
    }

    pub fn set_setting(&self, key: &str, value: Option<&str>) -> Result<(), String> {
        let connection = self.open_connection()?;

        match value {
            Some(value) => connection
                .execute(
                    "
                    INSERT INTO app_settings (key, value, updated_at)
                    VALUES (?1, ?2, ?3)
                    ON CONFLICT(key) DO UPDATE SET
                        value = excluded.value,
                        updated_at = excluded.updated_at
                    ",
                    params![key, value, current_time_ms()],
                )
                .map_err(|error| format!("Failed to write app setting: {}", error))?,
            None => connection
                .execute("DELETE FROM app_settings WHERE key = ?1", params![key])
                .map_err(|error| format!("Failed to delete app setting: {}", error))?,
        };

        Ok(())
    }

    pub fn get_all_settings(&self) -> Result<std::collections::HashMap<String, String>, String> {
        let connection = self.open_connection()?;
        let mut statement = connection
            .prepare("SELECT key, value FROM app_settings")
            .map_err(|error| format!("Failed to prepare all settings query: {}", error))?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                ))
            })
            .map_err(|error| format!("Failed to execute all settings query: {}", error))?;

        let mut map = std::collections::HashMap::new();
        for row in rows {
            let (key, value) =
                row.map_err(|error| format!("Failed to parse setting row: {}", error))?;
            map.insert(key, value);
        }

        Ok(map)
    }

    pub fn record_file_event(
        &self,
        path: &Path,
        source: FileSource,
    ) -> Result<(), String> {
        let snapshot = build_file_snapshot(path)?;
        let language = detect_file_language(path);
        let now = current_time_ms();
        let file_modified_app_at = Some(now);

        let connection = self.open_connection()?;

        connection
            .execute(
                "
                INSERT INTO tracked_files (
                    path_key,
                    path,
                    file_name,
                    extension,
                    source,
                    size_bytes,
                    file_modified_disk_at,
                    file_modified_app_at,
                    language,
                    created_at,
                    updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
                ON CONFLICT(path_key) DO UPDATE SET
                    path = excluded.path,
                    file_name = excluded.file_name,
                    extension = excluded.extension,
                    source = excluded.source,
                    size_bytes = excluded.size_bytes,
                    file_modified_disk_at = excluded.file_modified_disk_at,
                    file_modified_app_at = excluded.file_modified_app_at,
                    language = COALESCE(excluded.language, tracked_files.language),
                    updated_at = excluded.updated_at
                ",
                params![
                    snapshot.path_key,
                    snapshot.path,
                    snapshot.file_name,
                    snapshot.extension,
                    source.as_str(),
                    snapshot.size_bytes.map(|value| value as i64),
                    snapshot.file_modified_disk_at,
                    file_modified_app_at,
                    language,
                    now,
                ],
            )
            .map_err(|error| format!("Failed to upsert tracked file: {}", error))?;

        Ok(())
    }

    pub fn refresh_tracked_file(&self, path: &Path, source: FileSource) -> Result<(), String> {
        let snapshot = build_file_snapshot(path)?;
        let language = detect_file_language(path);
        let connection = self.open_connection()?;

        let now = current_time_ms();
        connection
            .execute(
                "
                UPDATE tracked_files
                SET
                    path = ?2,
                    file_name = ?3,
                    extension = ?4,
                    source = ?5,
                    size_bytes = ?6,
                    file_modified_disk_at = ?7,
                    language = COALESCE(?8, language),
                    updated_at = ?9
                WHERE path_key = ?1
                ",
                params![
                    snapshot.path_key,
                    snapshot.path,
                    snapshot.file_name,
                    snapshot.extension,
                    source.as_str(),
                    snapshot.size_bytes.map(|value| value as i64),
                    snapshot.file_modified_disk_at,
                    language,
                    now,
                ],
            )
            .map_err(|error| format!("Failed to refresh tracked file: {}", error))?;

        Ok(())
    }

    pub fn list_recent_files(&self, limit: usize) -> Result<Vec<RecentFileRecord>, String> {
        let connection = self.open_connection()?;
        let mut statement = connection
            .prepare(
                "
                SELECT
                    path,
                    file_name,
                    extension,
                    language,
                    source,
                    size_bytes,
                    file_modified_app_at,
                    file_modified_disk_at,
                    updated_at
                FROM tracked_files
                ORDER BY
                    COALESCE(file_modified_app_at, updated_at, 0) DESC,
                    updated_at DESC
                LIMIT ?1
                ",
            )
            .map_err(|error| format!("Failed to prepare recent files query: {}", error))?;

        let rows = statement
            .query_map(params![limit as i64], |row| {
                Ok(RecentFileRecord {
                    path: row.get(0)?,
                    file_name: row.get(1)?,
                    extension: row.get(2)?,
                    language: row.get(3)?,
                    source: row.get(4)?,
                    size_bytes: row.get::<_, Option<i64>>(5)?.map(|value| value as u64),
                    file_modified_app_at: row.get(6)?,
                    file_modified_disk_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })
            .map_err(|error| format!("Failed to execute recent files query: {}", error))?;

        let mut recent_files = Vec::new();
        for row in rows {
            recent_files
                .push(row.map_err(|error| format!("Failed to parse recent file row: {}", error))?);
        }

        Ok(recent_files)
    }

    pub fn get_tracked_file(&self, path: &Path) -> Result<Option<RecentFileRecord>, String> {
        let path_key = normalize_path_key(path)?;
        let connection = self.open_connection()?;

        connection
            .query_row(
                "
                SELECT
                    path,
                    file_name,
                    extension,
                    language,
                    source,
                    size_bytes,
                    file_modified_app_at,
                    file_modified_disk_at,
                    updated_at
                FROM tracked_files
                WHERE path_key = ?1
                ",
                params![path_key],
                |row| {
                    Ok(RecentFileRecord {
                        path: row.get(0)?,
                        file_name: row.get(1)?,
                        extension: row.get(2)?,
                        language: row.get(3)?,
                        source: row.get(4)?,
                        size_bytes: row.get::<_, Option<i64>>(5)?.map(|value| value as u64),
                        file_modified_app_at: row.get(6)?,
                        file_modified_disk_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(|error| format!("Failed to read tracked file: {}", error))
    }

    pub fn delete_tracked_file(&self, path: &Path) -> Result<(), String> {
        let path_key = normalize_path_key(path)?;
        let connection = self.open_connection()?;
        connection
            .execute(
                "DELETE FROM tracked_files WHERE path_key = ?1",
                params![path_key],
            )
            .map_err(|error| format!("Failed to delete tracked file: {}", error))?;
        Ok(())
    }

    /// Rename a tracked file in the database.
    /// Uses the old path_key to find the row and updates it with the new path
    /// and metadata. If no row exists, inserts a fresh one.
    pub fn rename_tracked_file(&self, old_path: &Path, new_path: &Path) -> Result<(), String> {
        let old_key = normalize_path_key(old_path)?;
        let new_snapshot = build_file_snapshot(new_path)?;
        let language = detect_file_language(new_path);
        let now = current_time_ms();

        let connection = self.open_connection()?;

        let updated = connection
            .execute(
                "
                UPDATE tracked_files SET
                    path_key = ?2,
                    path     = ?3,
                    file_name = ?4,
                    extension = ?5,
                    size_bytes = ?6,
                    file_modified_disk_at = ?7,
                    language = ?8,
                    updated_at = ?9
                WHERE path_key = ?1
                ",
                params![
                    old_key,
                    new_snapshot.path_key,
                    new_snapshot.path,
                    new_snapshot.file_name,
                    new_snapshot.extension,
                    new_snapshot.size_bytes.map(|v| v as i64),
                    new_snapshot.file_modified_disk_at,
                    language,
                    now,
                ],
            )
            .map_err(|e| format!("Failed to update renamed file row: {}", e))?;

        if updated == 0 {
            connection
                .execute(
                    "
                    INSERT INTO tracked_files (
                        path_key, path, file_name, extension, source,
                        size_bytes, file_modified_disk_at,
                        language, created_at, updated_at
                    )
                    VALUES (?1, ?2, ?3, ?4, 'slates', ?5, ?6, ?7, ?8, ?8)
                    ",
                    params![
                        new_snapshot.path_key,
                        new_snapshot.path,
                        new_snapshot.file_name,
                        new_snapshot.extension,
                        new_snapshot.size_bytes.map(|v| v as i64),
                        new_snapshot.file_modified_disk_at,
                        language,
                        now,
                    ],
                )
                .map_err(|e| format!("Failed to insert renamed file row: {}", e))?;
        }

        Ok(())
    }

    /// Returns a map of `path_key → path` for every tracked file whose source is
    /// `Slates`.  Used by the notes-directory sync to quickly determine which
    /// on-disk files are already known to the tracker.
    pub fn list_slates_path_map(&self) -> Result<HashMap<String, String>, String> {
        let connection = self.open_connection()?;
        let mut statement = connection
            .prepare(
                "SELECT path_key, path FROM tracked_files WHERE source = 'slates'",
            )
            .map_err(|error| format!("Failed to prepare slates path map query: {}", error))?;

        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| format!("Failed to execute slates path map query: {}", error))?;

        let mut map = HashMap::new();
        for row in rows {
            let (key, path) = row
                .map_err(|error| format!("Failed to parse slates path map row: {}", error))?;
            map.insert(key, path);
        }

        Ok(map)
    }

    /// Inserts or updates a tracked-file row for a file discovered during a
    /// sync scan of the notes directory.  Unlike `record_file_event`, this
    /// method does **not** create a `file_access_events` row and leaves
    /// `file_modified_app_at` untouched so the sync scan does
    /// not pollute open/save-based ordering.
    pub fn upsert_slates_file_for_sync(&self, path: &Path) -> Result<(), String> {
        let snapshot = build_file_snapshot(path)?;
        let language = detect_file_language(path);
        let now = current_time_ms();

        let connection = self.open_connection()?;
        connection
            .execute(
                "
                INSERT INTO tracked_files (
                    path_key,
                    path,
                    file_name,
                    extension,
                    source,
                    size_bytes,
                    file_modified_disk_at,
                    language,
                    created_at,
                    updated_at
                )
                VALUES (?1, ?2, ?3, ?4, 'slates', ?5, ?6, ?7, ?8, ?8)
                ON CONFLICT(path_key) DO UPDATE SET
                    path                 = excluded.path,
                    file_name            = excluded.file_name,
                    extension            = excluded.extension,
                    source               = 'slates',
                    size_bytes           = excluded.size_bytes,
                    file_modified_disk_at = excluded.file_modified_disk_at,
                    language             = COALESCE(excluded.language, tracked_files.language),
                    updated_at           = excluded.updated_at
                ",
                params![
                    snapshot.path_key,
                    snapshot.path,
                    snapshot.file_name,
                    snapshot.extension,
                    snapshot.size_bytes.map(|value| value as i64),
                    snapshot.file_modified_disk_at,
                    language,
                    now,
                ],
            )
            .map_err(|error| format!("Failed to upsert slates file for sync: {}", error))?;

        Ok(())
    }

    pub fn list_tracked_files(&self) -> Result<Vec<RecentFileRecord>, String> {
        let connection = self.open_connection()?;
        let mut statement = connection
            .prepare(
                "
                SELECT
                    path,
                    file_name,
                    extension,
                    language,
                    source,
                    size_bytes,
                    file_modified_app_at,
                    file_modified_disk_at,
                    updated_at
                FROM tracked_files
                ORDER BY updated_at DESC
                ",
            )
            .map_err(|error| format!("Failed to prepare tracked files query: {}", error))?;

        let rows = statement
            .query_map([], |row| {
                Ok(RecentFileRecord {
                    path: row.get(0)?,
                    file_name: row.get(1)?,
                    extension: row.get(2)?,
                    language: row.get(3)?,
                    source: row.get(4)?,
                    size_bytes: row.get::<_, Option<i64>>(5)?.map(|value| value as u64),
                    file_modified_app_at: row.get(6)?,
                    file_modified_disk_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })
            .map_err(|error| format!("Failed to execute tracked files query: {}", error))?;

        let mut tracked_files = Vec::new();
        for row in rows {
            tracked_files
                .push(row.map_err(|error| format!("Failed to parse tracked file row: {}", error))?);
        }

        Ok(tracked_files)
    }

    fn open_connection(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.db_path)
            .map_err(|error| format!("Failed to open SQLite database: {}", error))?;

        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| format!("Failed to enable WAL mode: {}", error))?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|error| format!("Failed to enable foreign keys: {}", error))?;
        connection
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(|error| format!("Failed to tune SQLite synchronous mode: {}", error))?;

        Ok(connection)
    }

    fn run_migrations(&self) -> Result<(), String> {
        let connection = self.open_connection()?;
        connection
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS app_settings (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS tracked_files (
                    path_key TEXT PRIMARY KEY,
                    path TEXT NOT NULL,
                    file_name TEXT NOT NULL,
                    extension TEXT,
                    language TEXT,
                    source TEXT NOT NULL CHECK (source IN ('slates', 'local')),
                    size_bytes INTEGER,
                    file_modified_app_at INTEGER,
                    file_modified_disk_at INTEGER,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_tracked_files_recent
                    ON tracked_files(file_modified_app_at DESC, updated_at DESC);

                CREATE INDEX IF NOT EXISTS idx_tracked_files_modified
                    ON tracked_files(file_modified_disk_at DESC, updated_at DESC);
                ",
            )
            .map_err(|error| format!("Failed to run SQLite migrations: {}", error))
    }
}

pub fn normalize_path_key(path: &Path) -> Result<String, String> {
    let normalized = normalize_path_buf(path)?;
    #[cfg(windows)]
    let key = normalized
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();

    #[cfg(not(windows))]
    let key = normalized.to_string_lossy().replace('\\', "/");

    Ok(key)
}

fn normalize_path_buf(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("Tracked file path must be absolute.".to_string());
    }

    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    Ok(normalized)
}

fn build_file_snapshot(path: &Path) -> Result<FileMetadataSnapshot, String> {
    let normalized = normalize_path_buf(path)?;
    let path_key = normalize_path_key(&normalized)?;
    let metadata = fs::metadata(&normalized).ok();

    let file_name = normalized
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| normalized.to_string_lossy().into_owned());

    let extension = normalized
        .extension()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty());

    Ok(FileMetadataSnapshot {
        path: normalized.to_string_lossy().into_owned(),
        path_key,
        file_name,
        extension,
        size_bytes: metadata.as_ref().map(|value| value.len()),
        file_modified_disk_at: metadata
            .and_then(|value| value.modified().ok())
            .and_then(system_time_to_ms),
    })
}

fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn system_time_to_ms(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as i64)
}

/// Detect language for a file path using extension-based heuristics.
/// Falls back to `"text"` for unrecognised files so the DB never stores NULL.
fn detect_file_language(path: &Path) -> String {
    let lang = path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| crate::detection::extension::detect_by_filename(n));
    lang.unwrap_or("text").to_string()
}
