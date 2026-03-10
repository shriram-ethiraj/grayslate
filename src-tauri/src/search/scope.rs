use std::{collections::HashMap, path::PathBuf};

use tauri::AppHandle;

use crate::{
    filesystem::resolve_notes_root_path,
    storage::{AppStorage, RecentFileRecord, normalize_path_key},
};

pub struct SearchScope {
    pub internal_root: Option<PathBuf>,
    pub external_files: Vec<PathBuf>,
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
        .map(|record| Ok((normalize_path_key(PathBuf::from(&record.path).as_path())?, record.clone())))
        .collect::<Result<HashMap<_, _>, String>>()?;

    let include_internal = matches!(filter_mode, "unified" | "internal");
    let include_external = matches!(filter_mode, "unified" | "external");
    let internal_root = if include_internal {
        let root = resolve_notes_root_path(app, storage)?;
        root.exists().then_some(root)
    } else {
        None
    };

    let external_files = if include_external {
        tracked_files
            .into_iter()
            .filter(|record| record.source == "external" && record.exists_on_disk)
            .map(|record| PathBuf::from(record.path))
            .collect()
    } else {
        Vec::new()
    };

    Ok(SearchScope {
        internal_root,
        external_files,
        tracked_by_key,
    })
}