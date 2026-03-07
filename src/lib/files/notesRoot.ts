import { normalize } from "@tauri-apps/api/path";
import { invoke } from "@tauri-apps/api/core";

const NOTES_ROOT_STORAGE_KEY = "grayslate.notesRoot";

function canUseLocalStorage(): boolean {
  return typeof window !== "undefined" && typeof window.localStorage !== "undefined";
}

export function getConfiguredNotesRoot(): string | null {
  if (!canUseLocalStorage()) {
    return null;
  }

  const storedPath = window.localStorage.getItem(NOTES_ROOT_STORAGE_KEY);
  return storedPath && storedPath.length > 0 ? storedPath : null;
}

export function setConfiguredNotesRoot(path: string | null): void {
  if (!canUseLocalStorage()) {
    return;
  }

  if (!path) {
    window.localStorage.removeItem(NOTES_ROOT_STORAGE_KEY);
    return;
  }

  window.localStorage.setItem(NOTES_ROOT_STORAGE_KEY, path);
}

export async function resolveNotesRoot(): Promise<string> {
  const configuredPath = getConfiguredNotesRoot();
  if (configuredPath) {
    return normalize(configuredPath);
  }

  return invoke<string>("resolve_default_notes_root");
}