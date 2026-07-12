use std::path::{Path, PathBuf};

use tauri::path::BaseDirectory;
use tauri::Manager;

use crate::storage::{normalize_path_key, AppStorage, FileSource, SETTING_NOTES_ROOT};

const MANAGED_NOTES_DIRECTORY: &str = "Grayslate";

pub fn resolve_default_notes_root_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let documents_dir = app
        .path()
        .resolve("", BaseDirectory::Document)
        .map_err(|error| format!("Unable to locate the Documents directory: {}", error))?;

    Ok(documents_dir.join(MANAGED_NOTES_DIRECTORY))
}

pub fn resolve_notes_root_path(
    app: &tauri::AppHandle,
    storage: &AppStorage,
) -> Result<PathBuf, String> {
    if let Some(configured_path) = storage.get_setting(SETTING_NOTES_ROOT)? {
        let configured_path = PathBuf::from(configured_path);
        if !configured_path.is_absolute() {
            return Err("Configured notes root must be an absolute path.".to_string());
        }

        return Ok(configured_path);
    }

    resolve_default_notes_root_path(app)
}

pub fn classify_file_source(
    app: &tauri::AppHandle,
    storage: &AppStorage,
    path: &Path,
) -> Result<FileSource, String> {
    let notes_root = resolve_notes_root_path(app, storage)?;
    let notes_root_key = normalize_path_key(&notes_root)?;
    let path_key = normalize_path_key(path)?;
    let is_slates = path_key == notes_root_key || path_key.starts_with(&(notes_root_key + "/"));

    Ok(if is_slates {
        FileSource::Slates
    } else {
        FileSource::Local
    })
}

/// Returns a path in `dir` based on `base_name` that does not already exist on
/// disk.  If `base_name` is taken, `-2`, `-3`, … are appended before the
/// last extension: `"note.md"` → `"note-2.md"`.
pub fn unique_path_in_dir(dir: &Path, base_name: &str) -> PathBuf {
    unique_path_in_dir_excluding(dir, base_name, None)
}

/// Returns a unique path in `dir`, optionally ignoring one existing path.
///
/// The exclusion is used by rename flows so the source file does not count as
/// a collision when the user submits its current filename unchanged.
pub fn unique_path_in_dir_excluding(
    dir: &Path,
    base_name: &str,
    excluded_path: Option<&Path>,
) -> PathBuf {
    let candidate = dir.join(base_name);
    if (!candidate.exists()) || excluded_path == Some(candidate.as_path()) {
        return candidate;
    }

    let (stem, ext) = if let Some(pos) = base_name.rfind('.') {
        (&base_name[..pos], &base_name[pos..])
    } else {
        (base_name, "")
    };

    let mut counter = 2u32;
    loop {
        let name = format!("{}-{}{}", stem, counter, ext);
        let path = dir.join(&name);
        if (!path.exists()) || excluded_path == Some(path.as_path()) {
            return path;
        }
        counter += 1;
    }
}

/// Sanitize and slugify a user-supplied filename for cross-platform safety.
///
/// - Strips leading/trailing whitespace.
/// - Preserves the file extension verbatim (from the last `.` onward).
/// - Replaces any run of whitespace, ASCII control characters, or Windows-
///   forbidden characters (`\ / : * ? " < > |`) with a single `-`.
/// - Never produces a leading or trailing hyphen.
/// - Returns an empty string when no safe characters remain.
pub fn sanitize_filename(name: &str) -> String {
    let trimmed = name.trim();

    // A leading dot (e.g. ".gitignore") is treated as stem-only — no extension.
    let (stem, ext) = match trimmed.rfind('.') {
        Some(pos) if pos > 0 => (&trimmed[..pos], &trimmed[pos..]),
        _ => (trimmed, ""),
    };

    let mut result = String::with_capacity(stem.len());
    let mut pending_sep = false;

    for ch in stem.chars() {
        // Windows is the most restrictive common OS: forbidden chars are \ / : * ? " < > |
        let is_unsafe = ch.is_control()
            || ch.is_whitespace()
            || matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|');

        if is_unsafe {
            // Collapse any run of unsafe chars into a single pending hyphen.
            pending_sep = true;
        } else {
            if pending_sep && !result.is_empty() {
                result.push('-');
            }
            pending_sep = false;
            result.push(ch);
        }
    }

    if result.is_empty() {
        return String::new();
    }

    format!("{}{}", result, ext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "grayslate_filesystem_test_{timestamp}_{count}"
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn unique_path_excluding_source_keeps_unchanged_name() {
        let dir = temp_dir();
        let source = dir.join("test.txt");
        fs::write(&source, "content").unwrap();

        let path = unique_path_in_dir_excluding(&dir, "test.txt", Some(&source));

        assert_eq!(path, source);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unique_path_excluding_source_still_suffixes_other_collision() {
        let dir = temp_dir();
        let source = dir.join("source.txt");
        let occupied = dir.join("test.txt");
        fs::write(&source, "source").unwrap();
        fs::write(&occupied, "occupied").unwrap();

        let path = unique_path_in_dir_excluding(&dir, "test.txt", Some(&source));

        assert_eq!(path, dir.join("test-2.txt"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unique_path_without_exclusion_keeps_existing_collision_behavior() {
        let dir = temp_dir();
        let occupied = dir.join("test.txt");
        fs::write(&occupied, "occupied").unwrap();

        let path = unique_path_in_dir(&dir, "test.txt");

        assert_eq!(path, dir.join("test-2.txt"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
