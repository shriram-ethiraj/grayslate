import { invoke } from "@tauri-apps/api/core";

export const OPEN_FILE_PATH_EVENT = "files://open-path";
export const RECENT_FILES_UPDATED_EVENT = "files://recent-updated";

export type RecentFileSource = "internal" | "external";

export interface RecentFileRecord {
  path: string;
  file_name: string;
  extension: string | null;
  source: RecentFileSource;
  exists_on_disk: boolean;
  size_bytes: number | null;
  last_opened_at: number | null;
  last_saved_at: number | null;
  last_seen_at: number | null;
  last_modified_at: number | null;
  pinned: boolean;
}

export interface SidebarSearchResult extends RecentFileRecord {
  preview_line: string | null;
  preview_line_number: number | null;
  match_count: number;
  filename_score: number;
  content_score: number;
  freshness_score: number;
  usage_score: number;
  final_score: number;
}

export interface OpenFilePathPayload {
  path: string;
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