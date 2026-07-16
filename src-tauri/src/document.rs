use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::Serialize;
use uuid::Uuid;

use crate::{
    filesystem::resolve_notes_root_path,
    storage::{AppStorage, FileSource},
};

const INVALID_GRANT: &str = "Document authorization is invalid or expired.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentAccess {
    Read,
    Write,
    Manage,
}

#[derive(Clone, Copy, Debug)]
pub struct DocumentRights {
    pub read: bool,
    pub write: bool,
    pub manage: bool,
}

impl DocumentRights {
    pub const fn tracked(source: FileSource) -> Self {
        Self {
            read: true,
            write: true,
            manage: matches!(source, FileSource::Slates),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AuthorizedDocument {
    pub id: String,
    pub generation: u64,
    pub path: PathBuf,
    pub source: FileSource,
    pub rights: DocumentRights,
    pub exists: bool,
}

impl AuthorizedDocument {
    pub fn descriptor(&self) -> DocumentDescriptor {
        DocumentDescriptor {
            document_id: self.id.clone(),
            generation: self.generation,
            display_path: self.path.to_string_lossy().into_owned(),
            file_name: self
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            source: self.source.as_str().to_string(),
            writable: self.rights.write,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDescriptor {
    pub document_id: String,
    pub generation: u64,
    pub display_path: String,
    pub file_name: String,
    pub source: String,
    pub writable: bool,
}

#[derive(Clone, Debug)]
struct RegistryEntry {
    window_label: String,
    document: AuthorizedDocument,
}

#[derive(Default)]
struct RegistryState {
    by_id: HashMap<String, RegistryEntry>,
    by_window_path: HashMap<(String, PathBuf), String>,
}

#[derive(Default)]
pub struct DocumentRegistry {
    state: Mutex<RegistryState>,
}

pub fn canonical_notes_root(
    app: &tauri::AppHandle,
    storage: &AppStorage,
    create: bool,
) -> Result<PathBuf, String> {
    let configured = resolve_notes_root_path(app, storage)?;
    if create && !configured.exists() {
        std::fs::create_dir_all(&configured)
            .map_err(|error| format!("Failed to create notes directory: {error}"))?;
    }
    let metadata = std::fs::symlink_metadata(&configured)
        .map_err(|error| format!("Cannot inspect notes directory: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Notes root must be a non-symlink directory.".to_string());
    }
    let canonical = std::fs::canonicalize(configured)
        .map_err(|error| format!("Cannot resolve notes directory: {error}"))?;
    if canonical.parent().is_none() {
        return Err("Notes root cannot be a filesystem root.".to_string());
    }
    let home = if cfg!(windows) {
        std::env::var_os("USERPROFILE")
    } else {
        std::env::var_os("HOME")
    };
    if let Some(home) = home {
        if std::fs::canonicalize(home).is_ok_and(|home| home == canonical) {
            return Err("Notes root cannot be the user profile directory.".to_string());
        }
    }
    Ok(canonical)
}

pub fn classify_existing_document(
    app: &tauri::AppHandle,
    storage: &AppStorage,
    path: &Path,
) -> Result<(PathBuf, FileSource), String> {
    let canonical = validate_existing_regular_file(path)?;
    let source = match canonical_notes_root(app, storage, false) {
        Ok(root) if canonical.starts_with(&root) => FileSource::Slates,
        _ => FileSource::Local,
    };
    Ok((canonical, source))
}

pub fn classify_new_document(
    app: &tauri::AppHandle,
    storage: &AppStorage,
    path: &Path,
) -> Result<(PathBuf, FileSource), String> {
    let candidate = validate_new_file_path(path)?;
    let source = match canonical_notes_root(app, storage, false) {
        Ok(root) if candidate.starts_with(&root) => FileSource::Slates,
        _ => FileSource::Local,
    };
    Ok((candidate, source))
}

pub fn revalidate_source_authority(
    app: &tauri::AppHandle,
    storage: &AppStorage,
    document: &AuthorizedDocument,
) -> Result<(), String> {
    if document.source != FileSource::Slates {
        return Ok(());
    }
    let root = canonical_notes_root(app, storage, false)?;
    if !document.path.starts_with(&root) || document.path == root {
        return Err("Managed document escaped the authorized notes root.".to_string());
    }
    Ok(())
}

pub fn open_authorized_read(path: &Path) -> Result<std::fs::File, String> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    options
        .open(path)
        .map_err(|error| format!("Failed to open authorized document: {error}"))
}

impl DocumentRegistry {
    pub fn grant_existing(
        &self,
        window_label: &str,
        path: &Path,
        source: FileSource,
        rights: DocumentRights,
    ) -> Result<AuthorizedDocument, String> {
        let canonical = validate_existing_regular_file(path)?;
        self.grant_canonical(window_label, canonical, source, rights, true)
    }

    pub fn grant_new(
        &self,
        window_label: &str,
        path: &Path,
        source: FileSource,
        rights: DocumentRights,
    ) -> Result<AuthorizedDocument, String> {
        let canonical = validate_new_file_path(path)?;
        self.grant_canonical(window_label, canonical, source, rights, false)
    }

    fn grant_canonical(
        &self,
        window_label: &str,
        path: PathBuf,
        source: FileSource,
        rights: DocumentRights,
        exists: bool,
    ) -> Result<AuthorizedDocument, String> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let path_key = (window_label.to_string(), path.clone());

        if let Some(id) = state.by_window_path.get(&path_key).cloned() {
            if let Some(entry) = state.by_id.get_mut(&id) {
                entry.document.source = source;
                entry.document.rights = rights;
                entry.document.exists = exists || entry.document.exists;
                return Ok(entry.document.clone());
            }
        }

        let id = Uuid::now_v7().to_string();
        let document = AuthorizedDocument {
            id: id.clone(),
            generation: 1,
            path,
            source,
            rights,
            exists,
        };
        state.by_window_path.insert(path_key, id.clone());
        state.by_id.insert(
            id,
            RegistryEntry {
                window_label: window_label.to_string(),
                document: document.clone(),
            },
        );
        Ok(document)
    }

    pub fn resolve(
        &self,
        window_label: &str,
        document_id: &str,
        generation: u64,
        access: DocumentAccess,
    ) -> Result<AuthorizedDocument, String> {
        let document = {
            let state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let entry = state.by_id.get(document_id).ok_or(INVALID_GRANT)?;
            if entry.window_label != window_label || entry.document.generation != generation {
                return Err(INVALID_GRANT.to_string());
            }
            entry.document.clone()
        };

        let allowed = match access {
            DocumentAccess::Read => document.rights.read,
            DocumentAccess::Write => document.rights.write,
            DocumentAccess::Manage => document.rights.manage,
        };
        if !allowed {
            return Err("This document grant does not allow that operation.".to_string());
        }

        revalidate_document_path(&document, access)?;
        Ok(document)
    }

    pub fn mark_created(
        &self,
        window_label: &str,
        document_id: &str,
        generation: u64,
    ) -> Result<AuthorizedDocument, String> {
        let document = {
            let state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let entry = state.by_id.get(document_id).ok_or(INVALID_GRANT)?;
            if entry.window_label != window_label
                || entry.document.generation != generation
                || !entry.document.rights.write
            {
                return Err(INVALID_GRANT.to_string());
            }
            entry.document.clone()
        };
        let canonical = validate_existing_regular_file(&document.path)?;

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let entry = state.by_id.get_mut(document_id).ok_or(INVALID_GRANT)?;
        entry.document.path = canonical;
        entry.document.exists = true;
        Ok(entry.document.clone())
    }

    pub fn replace_path(
        &self,
        window_label: &str,
        document_id: &str,
        generation: u64,
        new_path: &Path,
    ) -> Result<AuthorizedDocument, String> {
        let canonical = validate_existing_regular_file(new_path)?;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let (old_key, new_key, updated) = {
            let entry = state.by_id.get_mut(document_id).ok_or(INVALID_GRANT)?;
            if entry.window_label != window_label || entry.document.generation != generation {
                return Err(INVALID_GRANT.to_string());
            }
            let old_key = (window_label.to_string(), entry.document.path.clone());
            entry.document.path = canonical.clone();
            entry.document.exists = true;
            entry.document.generation = entry.document.generation.saturating_add(1);
            let new_key = (window_label.to_string(), canonical);
            (old_key, new_key, entry.document.clone())
        };

        state.by_window_path.remove(&old_key);
        state
            .by_window_path
            .insert(new_key, document_id.to_string());
        Ok(updated)
    }

    pub fn revoke(&self, window_label: &str, document_id: &str) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let should_remove = state
            .by_id
            .get(document_id)
            .is_some_and(|entry| entry.window_label == window_label);
        if !should_remove {
            return;
        }
        if let Some(entry) = state.by_id.remove(document_id) {
            state
                .by_window_path
                .remove(&(window_label.to_string(), entry.document.path));
        }
    }
}

fn validate_existing_regular_file(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("Document path must be absolute.".to_string());
    }
    let link_metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("Cannot inspect document: {error}"))?;
    if link_metadata.file_type().is_symlink() || !link_metadata.is_file() {
        return Err("Document must be a regular, non-symlink file.".to_string());
    }
    std::fs::canonicalize(path).map_err(|error| format!("Cannot resolve document: {error}"))
}

fn validate_new_file_path(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("Document path must be absolute.".to_string());
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| "Document path must include a file name.".to_string())?;
    if file_name.is_empty() {
        return Err("Document path must include a file name.".to_string());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "Document path must have a parent directory.".to_string())?;
    let canonical_parent = std::fs::canonicalize(parent)
        .map_err(|error| format!("Cannot resolve document directory: {error}"))?;
    if !canonical_parent.is_dir() {
        return Err("Document parent must be a directory.".to_string());
    }
    let candidate = canonical_parent.join(file_name);
    match std::fs::symlink_metadata(&candidate) {
        Ok(_) => Err("A file already exists at the new document path.".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(candidate),
        Err(error) => Err(format!("Cannot inspect document: {error}")),
    }
}

fn revalidate_document_path(
    document: &AuthorizedDocument,
    access: DocumentAccess,
) -> Result<(), String> {
    if document.exists || matches!(access, DocumentAccess::Read | DocumentAccess::Manage) {
        let canonical = validate_existing_regular_file(&document.path)?;
        if canonical != document.path {
            return Err(INVALID_GRANT.to_string());
        }
        return Ok(());
    }

    let candidate = validate_new_file_path(&document.path)?;
    if candidate != document.path {
        return Err(INVALID_GRANT.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn forged_and_cross_window_ids_are_rejected() {
        let dir = temp_dir("grayslate-document-grant");
        let path = dir.join("note.txt");
        std::fs::write(&path, "safe").unwrap();
        let registry = DocumentRegistry::default();
        let granted = registry
            .grant_existing(
                "main",
                &path,
                FileSource::Local,
                DocumentRights::tracked(FileSource::Local),
            )
            .unwrap();

        assert!(registry
            .resolve("main", "forged", 1, DocumentAccess::Read)
            .is_err());
        assert!(registry
            .resolve(
                "other",
                &granted.id,
                granted.generation,
                DocumentAccess::Read,
            )
            .is_err());
        assert!(registry
            .resolve(
                "main",
                &granted.id,
                granted.generation,
                DocumentAccess::Manage,
            )
            .is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn new_grant_cannot_claim_an_existing_file() {
        let dir = temp_dir("grayslate-document-new-existing");
        let path = dir.join("occupied.txt");
        std::fs::write(&path, "occupied").unwrap();
        let registry = DocumentRegistry::default();

        assert!(registry
            .grant_new(
                "main",
                &path,
                FileSource::Slates,
                DocumentRights::tracked(FileSource::Slates),
            )
            .is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn revoked_grant_cannot_be_reused() {
        let dir = temp_dir("grayslate-document-revoke");
        let path = dir.join("note.txt");
        std::fs::write(&path, "safe").unwrap();
        let registry = DocumentRegistry::default();
        let granted = registry
            .grant_existing(
                "main",
                &path,
                FileSource::Local,
                DocumentRights::tracked(FileSource::Local),
            )
            .unwrap();
        registry.revoke("main", &granted.id);

        assert!(registry
            .resolve(
                "main",
                &granted.id,
                granted.generation,
                DocumentAccess::Read,
            )
            .is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn stale_generation_is_rejected_after_rename() {
        let dir = temp_dir("grayslate-document-generation");
        let old_path = dir.join("old.txt");
        let new_path = dir.join("new.txt");
        std::fs::write(&old_path, "safe").unwrap();
        let registry = DocumentRegistry::default();
        let granted = registry
            .grant_existing(
                "main",
                &old_path,
                FileSource::Slates,
                DocumentRights::tracked(FileSource::Slates),
            )
            .unwrap();
        std::fs::rename(&old_path, &new_path).unwrap();
        let renamed = registry
            .replace_path("main", &granted.id, granted.generation, &new_path)
            .unwrap();

        assert!(registry
            .resolve(
                "main",
                &granted.id,
                granted.generation,
                DocumentAccess::Read,
            )
            .is_err());
        assert!(registry
            .resolve(
                "main",
                &renamed.id,
                renamed.generation,
                DocumentAccess::Read,
            )
            .is_ok());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn access_rejects_a_path_replaced_by_symlink() {
        use std::os::unix::fs::symlink;

        let dir = temp_dir("grayslate-document-symlink");
        let path = dir.join("note.txt");
        let victim = dir.join("victim.txt");
        std::fs::write(&path, "safe").unwrap();
        std::fs::write(&victim, "secret").unwrap();
        let registry = DocumentRegistry::default();
        let granted = registry
            .grant_existing(
                "main",
                &path,
                FileSource::Local,
                DocumentRights::tracked(FileSource::Local),
            )
            .unwrap();
        std::fs::remove_file(&path).unwrap();
        symlink(&victim, &path).unwrap();

        assert!(registry
            .resolve(
                "main",
                &granted.id,
                granted.generation,
                DocumentAccess::Read,
            )
            .is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn access_rejects_a_parent_replaced_by_symlink() {
        use std::os::unix::fs::symlink;

        let dir = temp_dir("grayslate-document-parent-symlink");
        let parent = dir.join("documents");
        let moved_parent = dir.join("documents-moved");
        let attacker = dir.join("attacker");
        std::fs::create_dir_all(&parent).unwrap();
        std::fs::create_dir_all(&attacker).unwrap();
        let path = parent.join("note.txt");
        std::fs::write(&path, "safe").unwrap();
        std::fs::write(attacker.join("note.txt"), "secret").unwrap();
        let registry = DocumentRegistry::default();
        let granted = registry
            .grant_existing(
                "main",
                &path,
                FileSource::Local,
                DocumentRights::tracked(FileSource::Local),
            )
            .unwrap();
        std::fs::rename(&parent, &moved_parent).unwrap();
        symlink(&attacker, &parent).unwrap();

        assert!(registry
            .resolve(
                "main",
                &granted.id,
                granted.generation,
                DocumentAccess::Read,
            )
            .is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
