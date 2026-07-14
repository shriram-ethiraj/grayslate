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
pub const SETTING_STARTUP_BEHAVIOR: &str = "startup_behavior";
pub const SETTING_LAST_ACTIVE_FILE: &str = "last_active_file";
pub const SETTING_DEFAULT_INDENT_MODE: &str = "default_indent_mode";
pub const SETTING_DEFAULT_INDENT_SIZE: &str = "default_indent_size";
pub const SETTING_CONFIRM_BEFORE_DELETE: &str = "confirm_before_delete";

/// All app setting keys that the app validates/converts at the command layer.
/// Used for batch-loading at startup so the frontend doesn't need to hardcode them.
pub const ALL_SETTING_KEYS: &[&str] = &[
    SETTING_NOTES_ROOT,
    SETTING_THEME,
    SETTING_FONT_SIZE,
    SETTING_WORD_WRAP,
    SETTING_SIDEBAR_WIDTH,
    SETTING_SIDEBAR_OPEN,
    SETTING_STARTUP_BEHAVIOR,
    SETTING_LAST_ACTIVE_FILE,
    SETTING_DEFAULT_INDENT_MODE,
    SETTING_DEFAULT_INDENT_SIZE,
    SETTING_CONFIRM_BEFORE_DELETE,
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

    /// Inserts a file that was opened before it had a tracking row.
    ///
    /// Existing rows are intentionally left completely unchanged so reopening
    /// a slate or local file cannot bump its recency timestamps. Returns true
    /// only when a new row was inserted.
    pub fn record_file_open_if_untracked(
        &self,
        path: &Path,
        source: FileSource,
    ) -> Result<bool, String> {
        let snapshot = build_file_snapshot(path)?;
        let language = detect_file_language(path);
        let now = current_time_ms();
        let connection = self.open_connection()?;

        let inserted = connection
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
                ON CONFLICT(path_key) DO NOTHING
                ",
                params![
                    snapshot.path_key,
                    snapshot.path,
                    snapshot.file_name,
                    snapshot.extension,
                    source.as_str(),
                    snapshot.size_bytes.map(|value| value as i64),
                    snapshot.file_modified_disk_at,
                    Some(now),
                    language,
                    now,
                ],
            )
            .map_err(|error| format!("Failed to track opened file: {}", error))?;

        Ok(inserted == 1)
    }

    /// Records a file creation or content save and updates its app timestamp.
    pub fn record_file_update(
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
        let path_key = &snapshot.path_key;
        let connection = self.open_connection()?;

        // Only bump updated_at when something material actually changed.
        // This prevents the "recently opened" sort from jittering on every
        // refresh just because we re-stamped the row bookkeeping timestamp.
        let existing: Option<RecentFileRecord> = connection
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
            .map_err(|error| format!("Failed to read tracked file for refresh: {}", error))?;

        if let Some(existing) = existing {
            let unchanged = existing.path == snapshot.path
                && existing.file_name == snapshot.file_name
                && existing.extension == snapshot.extension
                && existing.source == source.as_str()
                && existing.size_bytes == snapshot.size_bytes
                && existing.file_modified_disk_at == snapshot.file_modified_disk_at
                && existing.language == language;
            if unchanged {
                return Ok(());
            }
        }

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
                    path_key,
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
                    COALESCE(file_modified_app_at, file_modified_disk_at, 0) DESC,
                    file_name ASC,
                    path_key ASC
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
    /// sync scan of the notes directory. Unlike `record_file_update`, this
    /// method leaves
    /// `file_modified_app_at` untouched so the sync scan does
    /// not pollute open/save-based ordering.
    ///
    /// When the row already exists and its material metadata is unchanged,
    /// `updated_at` is left alone so refreshes do not re-order the sidebar.
    pub fn upsert_slates_file_for_sync(&self, path: &Path) -> Result<(), String> {
        let snapshot = build_file_snapshot(path)?;
        let language = detect_file_language(path);
        let path_key = &snapshot.path_key;
        let connection = self.open_connection()?;

        let existing: Option<RecentFileRecord> = connection
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
            .map_err(|error| format!("Failed to read tracked file for sync: {}", error))?;

        if let Some(existing) = existing {
            let unchanged = existing.path == snapshot.path
                && existing.file_name == snapshot.file_name
                && existing.extension == snapshot.extension
                && existing.source == "slates"
                && existing.size_bytes == snapshot.size_bytes
                && existing.file_modified_disk_at == snapshot.file_modified_disk_at
                && existing.language == language;
            if unchanged {
                return Ok(());
            }
        }

        let now = current_time_ms();
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
                    path_key,
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

                CREATE INDEX IF NOT EXISTS idx_tracked_files_recency
                    ON tracked_files(COALESCE(file_modified_app_at, file_modified_disk_at, 0) DESC, file_name ASC, path_key ASC);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_storage() -> (AppStorage, PathBuf) {
        let pid = std::process::id();
        let counter = TEST_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "grayslate-storage-test-{}-{}-{}",
            pid, counter, timestamp
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let storage = AppStorage {
            db_path: dir.join("test.sqlite3"),
        };
        storage.run_migrations().expect("run migrations");
        (storage, dir)
    }

    fn write_file(path: &Path, content: &[u8]) {
        let mut file = std::fs::File::create(path).expect("create file");
        file.write_all(content).expect("write file");
    }

    #[test]
    fn list_recent_files_is_stable_across_refreshes_for_synced_files() {
        let (storage, dir) = temp_storage();

        let alpha = dir.join("alpha.txt");
        let beta = dir.join("beta.txt");
        let gamma = dir.join("gamma.txt");
        write_file(&alpha, b"alpha");
        write_file(&beta, b"beta");
        write_file(&gamma, b"gamma");

        // All synced (never opened in-app).
        storage.upsert_slates_file_for_sync(&alpha).unwrap();
        storage.upsert_slates_file_for_sync(&beta).unwrap();
        storage.upsert_slates_file_for_sync(&gamma).unwrap();

        let order1: Vec<String> = storage
            .list_recent_files(10)
            .unwrap()
            .into_iter()
            .map(|record| record.path)
            .collect();

        // Simulate repeated refreshes; nothing has changed, so order must not jitter.
        for _ in 0..5 {
            storage.refresh_tracked_file(&alpha, FileSource::Slates).unwrap();
            storage.refresh_tracked_file(&beta, FileSource::Slates).unwrap();
            storage.refresh_tracked_file(&gamma, FileSource::Slates).unwrap();
            storage.upsert_slates_file_for_sync(&alpha).unwrap();
            storage.upsert_slates_file_for_sync(&beta).unwrap();
            storage.upsert_slates_file_for_sync(&gamma).unwrap();
        }

        let order2: Vec<String> = storage
            .list_recent_files(10)
            .unwrap()
            .into_iter()
            .map(|record| record.path)
            .collect();

        assert_eq!(
            order1, order2,
            "recent-files order must stay constant across no-op refreshes"
        );

        // All three files are present and accounted for.
        let expected = [
            alpha.to_string_lossy().to_string(),
            beta.to_string_lossy().to_string(),
            gamma.to_string_lossy().to_string(),
        ];
        assert_eq!(order1.len(), 3);
        for path in &expected {
            assert!(order1.contains(path), "missing {}", path);
        }
    }

    #[test]
    fn refresh_tracked_file_is_noop_when_nothing_changed() {
        let (storage, dir) = temp_storage();
        let path = dir.join("noop.txt");
        write_file(&path, b"hello");

        storage
            .record_file_update(&path, FileSource::Slates)
            .unwrap();
        let before = storage.get_tracked_file(&path).unwrap().unwrap();

        std::thread::sleep(std::time::Duration::from_millis(20));
        storage
            .refresh_tracked_file(&path, FileSource::Slates)
            .unwrap();

        let after = storage.get_tracked_file(&path).unwrap().unwrap();
        assert_eq!(before.updated_at, after.updated_at);
        assert_eq!(before.size_bytes, after.size_bytes);
    }

    #[test]
    fn first_open_tracks_local_file_without_updating_it_on_reopen() {
        let (storage, dir) = temp_storage();
        let path = dir.join("local.txt");
        write_file(&path, b"original");

        assert!(storage
            .record_file_open_if_untracked(&path, FileSource::Local)
            .unwrap());

        let first = storage.get_tracked_file(&path).unwrap().unwrap();
        assert_eq!(first.source, "local");
        assert!(first.file_modified_app_at.is_some());

        std::thread::sleep(std::time::Duration::from_millis(50));
        write_file(&path, b"changed after first open");

        assert!(!storage
            .record_file_open_if_untracked(&path, FileSource::Local)
            .unwrap());

        let reopened = storage.get_tracked_file(&path).unwrap().unwrap();
        assert_eq!(reopened.file_modified_app_at, first.file_modified_app_at);
        assert_eq!(reopened.file_modified_disk_at, first.file_modified_disk_at);
        assert_eq!(reopened.size_bytes, first.size_bytes);
        assert_eq!(reopened.updated_at, first.updated_at);
    }

    #[test]
    fn refresh_tracked_file_bumps_updated_at_on_real_change() {
        let (storage, dir) = temp_storage();
        let path = dir.join("change.txt");
        write_file(&path, b"hello");

        storage
            .record_file_update(&path, FileSource::Slates)
            .unwrap();
        let before = storage.get_tracked_file(&path).unwrap().unwrap();

        // Wait long enough to guarantee a different ms timestamp.
        std::thread::sleep(std::time::Duration::from_millis(50));
        write_file(&path, b"hello, world!");
        storage
            .refresh_tracked_file(&path, FileSource::Slates)
            .unwrap();

        let after = storage.get_tracked_file(&path).unwrap().unwrap();
        assert!(after.updated_at > before.updated_at);
        assert_ne!(before.size_bytes, after.size_bytes);
    }

    #[test]
    fn upsert_slates_file_for_sync_is_noop_when_nothing_changed() {
        let (storage, dir) = temp_storage();
        let path = dir.join("slate-noop.txt");
        write_file(&path, b"synced");

        storage.upsert_slates_file_for_sync(&path).unwrap();
        let before = storage.get_tracked_file(&path).unwrap().unwrap();

        std::thread::sleep(std::time::Duration::from_millis(50));
        storage.upsert_slates_file_for_sync(&path).unwrap();

        let after = storage.get_tracked_file(&path).unwrap().unwrap();
        assert_eq!(before.updated_at, after.updated_at);
    }

    #[test]
    fn recency_order_uses_app_update_timestamp_then_disk_timestamp() {
        let (storage, dir) = temp_storage();

        let opened = dir.join("opened.txt");
        let untouched = dir.join("untouched.txt");
        write_file(&opened, b"opened");
        write_file(&untouched, b"untouched");

        // Insert both via sync first (no app timestamp).
        storage.upsert_slates_file_for_sync(&opened).unwrap();
        storage.upsert_slates_file_for_sync(&untouched).unwrap();

        // Now record a content save for one file. Its app timestamp should
        // push it to the top regardless of disk mtime.
        storage
            .record_file_update(&opened, FileSource::Slates)
            .unwrap();

        let recent = storage.list_recent_files(10).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].path, opened.to_string_lossy().to_string());
        assert_eq!(recent[1].path, untouched.to_string_lossy().to_string());
        assert!(recent[0].file_modified_app_at.is_some());
        assert!(recent[1].file_modified_app_at.is_none());
    }
}
