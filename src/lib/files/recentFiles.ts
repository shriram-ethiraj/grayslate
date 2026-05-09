import { invoke } from "@tauri-apps/api/core";

export const OPEN_FILE_PATH_EVENT = "files://open-path";
export const RECENT_FILES_UPDATED_EVENT = "files://recent-updated";

export type RecentFileSource = "slates" | "local";

export interface RecentFileRecord {
  path: string;
  file_name: string;
  extension: string | null;
  language: string;
  source: RecentFileSource;
  size_bytes: number | null;
  file_modified_app_at: number | null;
  file_modified_disk_at: number | null;
  updated_at: number;
}

export interface HighlightFragment {
  text: string;
  is_match: boolean;
}

export interface MatchedLine {
  line_number: number;
  fragments: HighlightFragment[];
}

export interface SidebarSearchResult extends RecentFileRecord {
  matched_lines: MatchedLine[];
  match_count: number;
  filename_fragments: HighlightFragment[];
  filename_score: number;
  content_score: number;
  freshness_score: number;
  usage_score: number;
  final_score: number;
}

export interface OpenFilePathPayload {
  path: string;
  source?: RecentFileSource;
  lineNumber?: number;
}

export interface SearchOptions {
  caseSensitive: boolean;
  wholeWord: boolean;
  useRegex: boolean;
}

export const DEFAULT_SEARCH_OPTIONS: SearchOptions = {
  caseSensitive: false,
  wholeWord: false,
  useRegex: false,
};

export async function getRecentFiles(limit = 50): Promise<RecentFileRecord[]> {
  return invoke<RecentFileRecord[]>("get_recent_files", { limit });
}

export async function searchSidebarFiles(
  query: string,
  filterMode: "unified" | RecentFileSource,
  requestId: number,
  searchOptions: SearchOptions = DEFAULT_SEARCH_OPTIONS,
  limit = 80,
): Promise<SidebarSearchResult[]> {
  return invoke<SidebarSearchResult[]>("search_sidebar_files", {
    query,
    filterMode,
    requestId,
    caseSensitive: searchOptions.caseSensitive,
    wholeWord: searchOptions.wholeWord,
    useRegex: searchOptions.useRegex,
    limit,
  });
}

/** Immediately cancel any in-flight sidebar search for this window. */
export async function cancelSidebarSearch(): Promise<void> {
  return invoke<void>("cancel_sidebar_search");
}

/** Permanently delete a slate file from disk and remove it from tracking. */
export async function deleteFile(path: string): Promise<void> {
  return invoke<void>("delete_file", { path });
}

/**
 * Rename a slate file.  `newName` is the bare filename (no path separators).
 * The backend auto-appends a numeric suffix on collision.
 * Returns the absolute path of the renamed file.
 */
export async function renameFile(path: string, newName: string): Promise<string> {
  return invoke<string>("rename_file", { path, newName });
}

/**
 * Duplicate a file, placing a copy in the same directory with a `(copy)` suffix.
 * Returns the absolute path of the new copy.
 */
export async function duplicateFile(path: string): Promise<string> {
  return invoke<string>("duplicate_file", { path });
}

/**
 * Duplicate a local file into the Grayslate slates directory as a slate file.
 * Returns the absolute path of the new copy.
 */
export async function duplicateLocalFileAsSlate(path: string): Promise<string> {
  return invoke<string>("duplicate_local_file_as_slate", { path });
}

/** Remove a local (external) file from sidebar tracking without deleting it from disk. */
export async function untrackLocalFile(path: string): Promise<void> {
  return invoke<void>("untrack_local_file", { path });
}