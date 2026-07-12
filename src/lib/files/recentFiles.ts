import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { toast } from "$lib/components/ui/sonner";
import { editorState } from "$lib/state/editor.svelte";
import {
  appSettingsState,
} from "$lib/state/appSettings.svelte";
import { openDeleteFileDialog } from "$lib/state/appDialogs.svelte";
import { reportLibraryMutation } from "$lib/state/librarySidebar.svelte";

export const OPEN_FILE_PATH_EVENT = "files://open-path";
export const RECENT_FILES_UPDATED_EVENT = "files://recent-updated";
/**
 * Resets the editor to a blank untitled slate without re-running the
 * unsaved-changes confirm gate. Emitted by callers (e.g. unlink) that have
 * already confirmed with the user before performing an action that requires
 * the reset, so EditorWrapper must not prompt a second time.
 */
export const RESET_TO_BLANK_EVENT = "files://reset-to-blank";

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
 * Delete a file and run the shared post-delete cleanup: if the deleted file was
 * the one open in the editor, reset to a fresh untitled slate; then surface a
 * success toast. Throws on failure so the caller can keep its own busy state
 * (e.g. the confirmation dialog) and decide how to report the error.
 */
export async function performFileDelete(file: RecentFileRecord): Promise<void> {
  const wasCurrentFile = file.path === editorState.currentFilePath;
  await deleteFile(file.path);
  reportLibraryMutation({ kind: "removed", path: file.path });
  if (wasCurrentFile) {
    // Reset the editor to a new untitled slate via the shared event bus.
    await emit("menu://new-file");
  }
  toast.success(`"${file.file_name}" was deleted.`);
}

/**
 * Entry point for every file-delete trigger. Honors the user's
 * "confirm before deleting" preference: when enabled, opens the confirmation
 * dialog; when disabled, deletes immediately (still with toast + editor reset).
 */
export function requestDeleteFile(file: RecentFileRecord): void {
  if (appSettingsState.confirmBeforeDelete) {
    openDeleteFileDialog(file);
    return;
  }
  void performFileDelete(file).catch((err) => {
    const msg = err instanceof Error ? err.message : String(err);
    toast.error(`Failed to delete: ${msg}`);
  });
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

/**
 * Unlink a local file and run the shared post-unlink cleanup: if the unlinked
 * file was the one open in the editor, reset to a fresh untitled slate; then
 * surface a success toast. Throws on failure so the caller can restore its
 * own optimistic UI state.
 *
 * Callers are responsible for running the unsaved-changes confirm gate
 * (`confirmBeforeLeavingDocument`) before calling this, when the file being
 * unlinked is the one currently open — this function no longer prompts, so
 * it must not run until the caller has the user's go-ahead.
 */
export async function performFileUnlink(file: RecentFileRecord): Promise<void> {
  const wasCurrentFile = file.path === editorState.currentFilePath;
  await untrackLocalFile(file.path);
  reportLibraryMutation({ kind: "removed", path: file.path });
  if (wasCurrentFile) {
    // Reset the editor to a new untitled slate. The caller already ran the
    // unsaved-changes confirm gate, so use the no-reprompt reset event
    // instead of "menu://new-file" (which would confirm a second time).
    await emit(RESET_TO_BLANK_EVENT);
  }
  toast.success(`"${file.file_name}" unlinked from sidebar.`);
}
