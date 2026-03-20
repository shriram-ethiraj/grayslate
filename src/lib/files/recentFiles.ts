import { invoke } from "@tauri-apps/api/core";

export const OPEN_FILE_PATH_EVENT = "files://open-path";
export const RECENT_FILES_UPDATED_EVENT = "files://recent-updated";

export type RecentFileSource = "slates" | "local";

export interface RecentFileRecord {
  path: string;
  file_name: string;
  extension: string | null;
  language: string | null;
  source: RecentFileSource;
  exists_on_disk: boolean;
  size_bytes: number | null;
  last_opened_at: number | null;
  last_saved_at: number | null;
  last_seen_at: number | null;
  last_modified_at: number | null;
  pinned: boolean;
}

export interface MatchedLine {
  line_number: number;
  line_text: string;
}

export interface SidebarSearchResult extends RecentFileRecord {
  matched_lines: MatchedLine[];
  match_count: number;
  filename_score: number;
  content_score: number;
  freshness_score: number;
  usage_score: number;
  final_score: number;
}

export interface OpenFilePathPayload {
  path: string;
  lineNumber?: number;
}

export async function getRecentFiles(limit = 50): Promise<RecentFileRecord[]> {
  return invoke<RecentFileRecord[]>("get_recent_files", { limit });
}

export async function prepareFileOpen(path: string): Promise<RecentFileRecord> {
  return invoke<RecentFileRecord>("prepare_file_open", { path });
}

export async function searchSidebarFiles(
  query: string,
  filterMode: "unified" | RecentFileSource,
  requestId: number,
  limit = 80,
): Promise<SidebarSearchResult[]> {
  return invoke<SidebarSearchResult[]>("search_sidebar_files", {
    query,
    filterMode,
    requestId,
    limit,
  });
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