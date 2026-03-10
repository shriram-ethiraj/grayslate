pub mod grep;
pub mod query;
pub mod rank;
pub mod scope;
pub mod types;

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use tauri::AppHandle;

use crate::{
    commands::search::SearchRuntimeState,
    storage::{AppStorage, RecentFileRecord, normalize_path_key},
};

use self::{
    grep::{collect_content_matches, list_scope_files},
    query::parse_query,
    rank::{RankContext, sort_search_results},
    scope::{resolve_search_scope},
    types::{FileSearchCandidate, SearchResultRecord, system_time_to_unix_ms},
};

pub fn run_sidebar_search(
    app: &AppHandle,
    storage: &AppStorage,
    runtime: &SearchRuntimeState,
    raw_query: &str,
    filter_mode: &str,
    limit: usize,
    cancelled: &AtomicBool,
) -> Result<Vec<SearchResultRecord>, String> {
    let parsed_query = parse_query(raw_query)?;
    if parsed_query.terms.is_empty() {
        return Ok(Vec::new());
    }

    let scope = resolve_search_scope(app, storage, filter_mode)?;

    // Walk the directory once and reuse the file list for both
    // candidate-path collection and content matching.
    let scope_files = list_scope_files(&scope, cancelled)?;

    let mut indexed_paths = HashSet::new();
    for path in &scope_files {
        indexed_paths.insert(normalize_path_key(path)?);
    }
    for path_key in scope.tracked_by_key.keys() {
        indexed_paths.insert(path_key.clone());
    }
    if indexed_paths.is_empty() {
        return Ok(Vec::new());
    }

    let content_matches = collect_content_matches(&scope_files, &parsed_query, cancelled)?;
    let mut candidates = build_candidates(indexed_paths, scope.tracked_by_key, content_matches)?;

    let average_document_length = rank::resolve_average_document_length(
        runtime.average_document_length(),
        candidates.values().map(|candidate| candidate.document_length),
    );
    runtime.update_average_document_length(Some(average_document_length));

    let rank_context = RankContext {
        query: &parsed_query,
        average_document_length,
        document_count: candidates.len(),
    };

    let mut results = candidates
        .values_mut()
        .filter_map(|candidate| rank::rank_candidate(candidate, &rank_context))
        .collect::<Vec<_>>();

    sort_search_results(&mut results);
    results.truncate(limit);
    Ok(results)
}



fn build_candidates(
    indexed_paths: HashSet<String>,
    tracked_by_key: HashMap<String, RecentFileRecord>,
    content_matches: HashMap<String, types::ContentMatchSummary>,
) -> Result<HashMap<String, FileSearchCandidate>, String> {
    let mut candidates = HashMap::new();

    for path_key in indexed_paths.into_iter().chain(content_matches.keys().cloned()) {
        if candidates.contains_key(&path_key) {
            continue;
        }

        let tracked = tracked_by_key.get(&path_key);
        let resolved_path = tracked
            .map(|record| PathBuf::from(&record.path))
            .or_else(|| restore_normalized_path(&path_key))
            .ok_or_else(|| format!("Failed to resolve search result path: {}", path_key))?;

        let metadata = fs::metadata(&resolved_path).ok();
        let exists_on_disk = metadata.is_some();
        let file_name = resolved_path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| resolved_path.to_string_lossy().into_owned());
        let extension = resolved_path
            .extension()
            .map(|value| value.to_string_lossy().into_owned())
            .filter(|value| !value.is_empty());
        let content = content_matches.get(&path_key).cloned().unwrap_or_default();
        let tracked = tracked.cloned();
        let size_bytes = metadata
            .as_ref()
            .map(|value| value.len())
            .or(tracked.as_ref().and_then(|value| value.size_bytes));
        let last_modified_at = metadata
            .and_then(|value| value.modified().ok())
            .and_then(system_time_to_unix_ms)
            .or(tracked.as_ref().and_then(|value| value.last_modified_at));

        candidates.insert(
            path_key,
            FileSearchCandidate {
                path: resolved_path.to_string_lossy().into_owned(),
                file_name,
                extension,
                source: tracked
                    .as_ref()
                    .map(|value| value.source.clone())
                    .unwrap_or_else(|| "internal".to_string()),
                exists_on_disk,
                size_bytes,
                last_opened_at: tracked.as_ref().and_then(|value| value.last_opened_at),
                last_saved_at: tracked.as_ref().and_then(|value| value.last_saved_at),
                last_seen_at: tracked.as_ref().and_then(|value| value.last_seen_at),
                last_modified_at,
                pinned: tracked.as_ref().map(|value| value.pinned).unwrap_or(false),
                content,
                document_length: size_bytes.unwrap_or(0).max(1) as f32,
            },
        );
    }

    Ok(candidates)
}

fn restore_normalized_path(path_key: &str) -> Option<PathBuf> {
    let path = Path::new(path_key);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }

    #[cfg(windows)]
    {
        let bytes = path_key.as_bytes();
        if bytes.len() >= 3 && bytes[1] == b':' && bytes[2] == b'/' {
            let mut chars = path_key.chars();
            let drive = chars.next()?.to_ascii_uppercase();
            let rest = &path_key[1..];
            return Some(PathBuf::from(format!("{}{}", drive, rest)));
        }
    }

    None
}