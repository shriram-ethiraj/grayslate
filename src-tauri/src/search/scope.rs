use std::{collections::HashMap, path::PathBuf};

use tauri::AppHandle;

use crate::{
    filesystem::resolve_notes_root_path,
    storage::{normalize_path_key, AppStorage, RecentFileRecord},
};

pub struct SearchScope {
    pub slates_root: Option<PathBuf>,
    pub local_files: Vec<PathBuf>,
    pub tracked_by_key: HashMap<String, RecentFileRecord>,
}

pub fn resolve_search_scope(
    app: &AppHandle,
    storage: &AppStorage,
    filter_mode: &str,
) -> Result<SearchScope, String> {
    let tracked_files = storage.list_tracked_files()?;
    let tracked_by_key = tracked_files
        .iter()
        .map(|record| {
            Ok((
                normalize_path_key(PathBuf::from(&record.path).as_path())?,
                record.clone(),
            ))
        })
        .collect::<Result<HashMap<_, _>, String>>()?;

    let include_slates = matches!(filter_mode, "unified" | "slates");
    let include_local = matches!(filter_mode, "unified" | "local");
    let slates_root = if include_slates {
        let root = resolve_notes_root_path(app, storage)?;
        root.exists().then_some(root)
    } else {
        None
    };

    let local_files = if include_local {
        tracked_files
            .into_iter()
            .filter(|record| record.source == "local" && record.exists_on_disk)
            .map(|record| PathBuf::from(record.path))
            .collect()
    } else {
        Vec::new()
    };

    Ok(SearchScope {
        slates_root,
        local_files,
        tracked_by_key,
    })
}
