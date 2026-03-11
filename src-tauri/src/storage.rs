use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::{path::BaseDirectory, AppHandle, Manager};

pub const SETTING_NOTES_ROOT: &str = "notes_root";

const DATABASE_FILENAME: &str = "grayslate.sqlite3";
const EVENT_OPEN: &str = "open";
const EVENT_SAVE: &str = "save";

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileEventType {
    Open,
    Save,
}

impl FileEventType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Open => EVENT_OPEN,
            Self::Save => EVENT_SAVE,
        }
    }
}

#[derive(Clone, Serialize)]
pub struct RecentFileRecord {
    pub path: String,
    pub file_name: String,
    pub extension: Option<String>,
    pub source: String,
    pub exists_on_disk: bool,
    pub size_bytes: Option<u64>,
    pub last_opened_at: Option<i64>,
    pub last_saved_at: Option<i64>,
    pub last_seen_at: Option<i64>,
    pub last_modified_at: Option<i64>,
    pub pinned: bool,
}

struct FileMetadataSnapshot {
    path: String,
    path_key: String,
    file_name: String,
    extension: Option<String>,
    exists_on_disk: bool,
    size_bytes: Option<u64>,
    last_seen_at: i64,
    last_modified_at: Option<i64>,
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

    pub fn record_file_event(
        &self,
        path: &Path,
        source: FileSource,
        event_type: FileEventType,
    ) -> Result<(), String> {
        let snapshot = build_file_snapshot(path)?;
        let now = current_time_ms();
        let last_opened_at = matches!(event_type, FileEventType::Open).then_some(now);
        let last_saved_at = matches!(event_type, FileEventType::Save).then_some(now);

        let mut connection = self.open_connection()?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("Failed to start file event transaction: {}", error))?;

        transaction
            .execute(
                "
                INSERT INTO tracked_files (
                    path_key,
                    path,
                    file_name,
                    extension,
                    source,
                    exists_on_disk,
                    size_bytes,
                    last_seen_at,
                    last_modified_at,
                    last_opened_at,
                    last_saved_at,
                    created_at,
                    updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
                ON CONFLICT(path_key) DO UPDATE SET
                    path = excluded.path,
                    file_name = excluded.file_name,
                    extension = excluded.extension,
                    source = excluded.source,
                    exists_on_disk = excluded.exists_on_disk,
                    size_bytes = excluded.size_bytes,
                    last_seen_at = excluded.last_seen_at,
                    last_modified_at = excluded.last_modified_at,
                    last_opened_at = COALESCE(excluded.last_opened_at, tracked_files.last_opened_at),
                    last_saved_at = COALESCE(excluded.last_saved_at, tracked_files.last_saved_at),
                    updated_at = excluded.updated_at
                ",
                params![
                    snapshot.path_key,
                    snapshot.path,
                    snapshot.file_name,
                    snapshot.extension,
                    source.as_str(),
                    snapshot.exists_on_disk as i64,
                    snapshot.size_bytes.map(|value| value as i64),
                    snapshot.last_seen_at,
                    snapshot.last_modified_at,
                    last_opened_at,
                    last_saved_at,
                    now,
                ],
            )
            .map_err(|error| format!("Failed to upsert tracked file: {}", error))?;

        let file_id: i64 = transaction
            .query_row(
                "SELECT id FROM tracked_files WHERE path_key = ?1",
                params![snapshot.path_key],
                |row| row.get(0),
            )
            .map_err(|error| format!("Failed to resolve tracked file id: {}", error))?;

        transaction
            .execute(
                "
                INSERT INTO file_access_events (file_id, event_type, occurred_at)
                VALUES (?1, ?2, ?3)
                ",
                params![file_id, event_type.as_str(), now],
            )
            .map_err(|error| format!("Failed to insert file access event: {}", error))?;

        transaction
            .commit()
            .map_err(|error| format!("Failed to commit file event transaction: {}", error))
    }

    pub fn refresh_tracked_file(&self, path: &Path, source: FileSource) -> Result<(), String> {
        let snapshot = build_file_snapshot(path)?;
        let connection = self.open_connection()?;

        connection
            .execute(
                "
                UPDATE tracked_files
                SET
                    path = ?2,
                    file_name = ?3,
                    extension = ?4,
                    source = ?5,
                    exists_on_disk = ?6,
                    size_bytes = ?7,
                    last_seen_at = ?8,
                    last_modified_at = ?9,
                    updated_at = ?8
                WHERE path_key = ?1
                ",
                params![
                    snapshot.path_key,
                    snapshot.path,
                    snapshot.file_name,
                    snapshot.extension,
                    source.as_str(),
                    snapshot.exists_on_disk as i64,
                    snapshot.size_bytes.map(|value| value as i64),
                    snapshot.last_seen_at,
                    snapshot.last_modified_at,
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
                    source,
                    exists_on_disk,
                    size_bytes,
                    last_opened_at,
                    last_saved_at,
                    last_seen_at,
                    last_modified_at,
                    pinned
                FROM tracked_files
                ORDER BY
                    pinned DESC,
                    COALESCE(last_opened_at, last_saved_at, last_seen_at, 0) DESC,
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
                    source: row.get(3)?,
                    exists_on_disk: row.get::<_, i64>(4)? != 0,
                    size_bytes: row.get::<_, Option<i64>>(5)?.map(|value| value as u64),
                    last_opened_at: row.get(6)?,
                    last_saved_at: row.get(7)?,
                    last_seen_at: row.get(8)?,
                    last_modified_at: row.get(9)?,
                    pinned: row.get::<_, i64>(10)? != 0,
                })
            })
            .map_err(|error| format!("Failed to execute recent files query: {}", error))?;

        let mut recent_files = Vec::new();
        for row in rows {
            recent_files.push(
                row.map_err(|error| format!("Failed to parse recent file row: {}", error))?,
            );
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
                    source,
                    exists_on_disk,
                    size_bytes,
                    last_opened_at,
                    last_saved_at,
                    last_seen_at,
                    last_modified_at,
                    pinned
                FROM tracked_files
                WHERE path_key = ?1
                ",
                params![path_key],
                |row| {
                    Ok(RecentFileRecord {
                        path: row.get(0)?,
                        file_name: row.get(1)?,
                        extension: row.get(2)?,
                        source: row.get(3)?,
                        exists_on_disk: row.get::<_, i64>(4)? != 0,
                        size_bytes: row.get::<_, Option<i64>>(5)?.map(|value| value as u64),
                        last_opened_at: row.get(6)?,
                        last_saved_at: row.get(7)?,
                        last_seen_at: row.get(8)?,
                        last_modified_at: row.get(9)?,
                        pinned: row.get::<_, i64>(10)? != 0,
                    })
                },
            )
            .optional()
            .map_err(|error| format!("Failed to read tracked file: {}", error))
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
                    source,
                    exists_on_disk,
                    size_bytes,
                    last_opened_at,
                    last_saved_at,
                    last_seen_at,
                    last_modified_at,
                    pinned
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
                    source: row.get(3)?,
                    exists_on_disk: row.get::<_, i64>(4)? != 0,
                    size_bytes: row.get::<_, Option<i64>>(5)?.map(|value| value as u64),
                    last_opened_at: row.get(6)?,
                    last_saved_at: row.get(7)?,
                    last_seen_at: row.get(8)?,
                    last_modified_at: row.get(9)?,
                    pinned: row.get::<_, i64>(10)? != 0,
                })
            })
            .map_err(|error| format!("Failed to execute tracked files query: {}", error))?;

        let mut tracked_files = Vec::new();
        for row in rows {
            tracked_files.push(
                row.map_err(|error| format!("Failed to parse tracked file row: {}", error))?,
            );
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
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    path_key TEXT NOT NULL UNIQUE,
                    path TEXT NOT NULL,
                    file_name TEXT NOT NULL,
                    extension TEXT,
                    source TEXT NOT NULL CHECK (source IN ('slates', 'local')),
                    exists_on_disk INTEGER NOT NULL DEFAULT 1,
                    size_bytes INTEGER,
                    last_opened_at INTEGER,
                    last_saved_at INTEGER,
                    last_seen_at INTEGER,
                    last_modified_at INTEGER,
                    pinned INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_tracked_files_recent
                    ON tracked_files(last_opened_at DESC, last_saved_at DESC, updated_at DESC);

                CREATE INDEX IF NOT EXISTS idx_tracked_files_modified
                    ON tracked_files(last_modified_at DESC, updated_at DESC);

                CREATE TABLE IF NOT EXISTS file_access_events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_id INTEGER NOT NULL,
                    event_type TEXT NOT NULL CHECK (event_type IN ('open', 'save')),
                    occurred_at INTEGER NOT NULL,
                    FOREIGN KEY(file_id) REFERENCES tracked_files(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_file_access_events_file_time
                    ON file_access_events(file_id, occurred_at DESC);

                CREATE TABLE IF NOT EXISTS transformations (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    key TEXT NOT NULL UNIQUE,
                    label TEXT NOT NULL,
                    category TEXT,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS transformation_events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    transformation_id INTEGER NOT NULL,
                    file_id INTEGER,
                    file_extension TEXT,
                    succeeded INTEGER NOT NULL DEFAULT 1,
                    duration_ms INTEGER,
                    error_message TEXT,
                    occurred_at INTEGER NOT NULL,
                    FOREIGN KEY(transformation_id) REFERENCES transformations(id) ON DELETE CASCADE,
                    FOREIGN KEY(file_id) REFERENCES tracked_files(id) ON DELETE SET NULL
                );

                CREATE INDEX IF NOT EXISTS idx_transformation_events_recent
                    ON transformation_events(occurred_at DESC);

                CREATE INDEX IF NOT EXISTS idx_transformation_events_file
                    ON transformation_events(file_id, occurred_at DESC);

                CREATE INDEX IF NOT EXISTS idx_transformation_events_extension
                    ON transformation_events(file_extension, occurred_at DESC);
                ",
            )
            .map_err(|error| format!("Failed to run SQLite migrations: {}", error))?;

        self.migrate_source_values()
    }

    /// One-time migration: renames legacy source values ('external'→'local',
    /// 'internal'→'slates') and updates the CHECK constraint via the official
    /// SQLite 12-step table-rebuild procedure.
    fn migrate_source_values(&self) -> Result<(), String> {
        let connection = self.open_connection()?;

        // Check whether any legacy rows exist.
        let needs_migration: bool = connection
            .query_row(
                "SELECT COUNT(*) > 0 FROM tracked_files WHERE source IN ('external', 'internal')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !needs_migration {
            return Ok(());
        }

        // Disable foreign-key enforcement for the duration of the table rebuild.
        connection
            .execute("PRAGMA foreign_keys = OFF", [])
            .map_err(|e| format!("Migration (FK off) failed: {}", e))?;

        let result = connection.execute_batch(
            "
            BEGIN;

            CREATE TABLE tracked_files_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path_key TEXT NOT NULL UNIQUE,
                path TEXT NOT NULL,
                file_name TEXT NOT NULL,
                extension TEXT,
                source TEXT NOT NULL CHECK (source IN ('slates', 'local')),
                exists_on_disk INTEGER NOT NULL DEFAULT 1,
                size_bytes INTEGER,
                last_opened_at INTEGER,
                last_saved_at INTEGER,
                last_seen_at INTEGER,
                last_modified_at INTEGER,
                pinned INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            INSERT INTO tracked_files_new
            SELECT
                id, path_key, path, file_name, extension,
                CASE source
                    WHEN 'external' THEN 'local'
                    WHEN 'internal' THEN 'slates'
                    ELSE source
                END,
                exists_on_disk, size_bytes, last_opened_at, last_saved_at,
                last_seen_at, last_modified_at, pinned, created_at, updated_at
            FROM tracked_files;

            DROP TABLE tracked_files;
            ALTER TABLE tracked_files_new RENAME TO tracked_files;

            CREATE INDEX IF NOT EXISTS idx_tracked_files_recent
                ON tracked_files(last_opened_at DESC, last_saved_at DESC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_tracked_files_modified
                ON tracked_files(last_modified_at DESC, updated_at DESC);

            COMMIT;
            ",
        );

        // Always re-enable foreign keys before surfacing any error.
        connection
            .execute("PRAGMA foreign_keys = ON", [])
            .map_err(|e| format!("Migration (FK on) failed: {}", e))?;

        result.map_err(|e| format!("Migration (source values) failed: {}", e))
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
        exists_on_disk: metadata.is_some(),
        size_bytes: metadata.as_ref().map(|value| value.len()),
        last_seen_at: current_time_ms(),
        last_modified_at: metadata
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