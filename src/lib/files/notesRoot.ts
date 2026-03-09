import { normalize } from "@tauri-apps/api/path";
import { invoke } from "@tauri-apps/api/core";

const NOTES_ROOT_SETTING_KEY = "notes_root";

export async function getConfiguredNotesRoot(): Promise<string | null> {
  const storedPath = await invoke<string | null>("get_app_setting", {
    key: NOTES_ROOT_SETTING_KEY,
  });

  return storedPath ? normalize(storedPath) : null;
}

export async function setConfiguredNotesRoot(path: string | null): Promise<void> {
  const normalizedPath = path ? await normalize(path) : null;

  await invoke("set_app_setting", {
    key: NOTES_ROOT_SETTING_KEY,
    value: normalizedPath,
  });
}

export async function resolveNotesRoot(): Promise<string> {
  return invoke<string>("resolve_notes_root");
}