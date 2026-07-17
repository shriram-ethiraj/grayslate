import { invoke } from "@tauri-apps/api/core";

const KEY_THEME = "theme";
const KEY_FONT_SIZE = "font_size";
const KEY_WORD_WRAP = "word_wrap";
const KEY_SIDEBAR_WIDTH = "sidebar_width";
const KEY_SIDEBAR_OPEN = "sidebar_open";
const KEY_STARTUP_BEHAVIOR = "startup_behavior";
const KEY_LAST_ACTIVE_FILE = "last_active_file";
const KEY_DEFAULT_INDENT_MODE = "default_indent_mode";
const KEY_DEFAULT_INDENT_SIZE = "default_indent_size";
const KEY_CONFIRM_BEFORE_DELETE = "confirm_before_delete";

export type ThemeSetting = "dark" | "light";
export type StartupBehavior = "new" | "last";
export type DefaultIndentMode = "spaces" | "tab";

export interface AppSettings {
    theme: ThemeSetting;
    fontSize: number;
    wordWrap: boolean;
    sidebarWidth: number;
    sidebarOpen: boolean;
    startupBehavior: StartupBehavior;
    // Internal bookkeeping: absolute path of the last-opened saved file, or null.
    // Not surfaced in the Settings UI — written by the editor, read on startup.
    lastActiveFile: string | null;
    defaultIndentMode: DefaultIndentMode;
    defaultIndentSize: number;
    confirmBeforeDelete: boolean;
}

export const DEFAULT_THEME: ThemeSetting = "dark";
export const DEFAULT_FONT_SIZE = 14;
export const DEFAULT_WORD_WRAP = false;
export const DEFAULT_SIDEBAR_WIDTH = 20;
export const DEFAULT_SIDEBAR_OPEN = false;
export const DEFAULT_STARTUP_BEHAVIOR: StartupBehavior = "new";
export const DEFAULT_DEFAULT_INDENT_MODE: DefaultIndentMode = "spaces";
export const DEFAULT_DEFAULT_INDENT_SIZE = 2;
export const DEFAULT_CONFIRM_BEFORE_DELETE = true;

export const DEFAULT_SETTINGS: AppSettings = {
    theme: DEFAULT_THEME,
    fontSize: DEFAULT_FONT_SIZE,
    wordWrap: DEFAULT_WORD_WRAP,
    sidebarWidth: DEFAULT_SIDEBAR_WIDTH,
    sidebarOpen: DEFAULT_SIDEBAR_OPEN,
    startupBehavior: DEFAULT_STARTUP_BEHAVIOR,
    lastActiveFile: null,
    defaultIndentMode: DEFAULT_DEFAULT_INDENT_MODE,
    defaultIndentSize: DEFAULT_DEFAULT_INDENT_SIZE,
    confirmBeforeDelete: DEFAULT_CONFIRM_BEFORE_DELETE,
};

const prefersDark = typeof window !== "undefined"
    ? window.matchMedia("(prefers-color-scheme: dark)").matches
    : true;

export async function loadAllSettings(): Promise<AppSettings> {
    const raw = await invoke<Record<string, string>>("get_all_settings");
    const storedTheme = raw[KEY_THEME];
    const theme: ThemeSetting = storedTheme === "light" || storedTheme === "dark"
        ? storedTheme
        : prefersDark ? "dark" : "light";
    const storedIndentSize = raw[KEY_DEFAULT_INDENT_SIZE]
        ? parseInt(raw[KEY_DEFAULT_INDENT_SIZE], 10)
        : DEFAULT_DEFAULT_INDENT_SIZE;
    return {
        theme,
        fontSize: raw[KEY_FONT_SIZE] ? parseInt(raw[KEY_FONT_SIZE], 10) : DEFAULT_FONT_SIZE,
        wordWrap: raw[KEY_WORD_WRAP] === "true",
        sidebarWidth: raw[KEY_SIDEBAR_WIDTH] ? parseInt(raw[KEY_SIDEBAR_WIDTH], 10) : DEFAULT_SIDEBAR_WIDTH,
        sidebarOpen: raw[KEY_SIDEBAR_OPEN] === "true",
        startupBehavior: raw[KEY_STARTUP_BEHAVIOR] === "last" ? "last" : "new",
        lastActiveFile: raw[KEY_LAST_ACTIVE_FILE] ?? null,
        defaultIndentMode: raw[KEY_DEFAULT_INDENT_MODE] === "tab" ? "tab" : "spaces",
        defaultIndentSize: Number.isFinite(storedIndentSize) ? storedIndentSize : DEFAULT_DEFAULT_INDENT_SIZE,
        confirmBeforeDelete: raw[KEY_CONFIRM_BEFORE_DELETE] !== "false",
    };
}

export async function saveSetting(key: string, value: string | null): Promise<void> {
    // A `null` value maps to Rust `Option::None`, which deletes the key.
    await invoke("set_app_setting", { key, value });
}

export function applyTheme(isDark: boolean): void {
    if (isDark) {
        document.documentElement.classList.add("dark");
    } else {
        document.documentElement.classList.remove("dark");
    }
    localStorage.setItem("theme", isDark ? "dark" : "light");
    saveSetting(KEY_THEME, isDark ? "dark" : "light");
}

export function getThemeFromLocalStorage(): ThemeSetting {
    const stored = localStorage.getItem("theme");
    if (stored === "light" || stored === "dark") return stored;
    return prefersDark ? "dark" : "light";
}

const debounceTimers = new Map<string, ReturnType<typeof setTimeout>>();

export function debouncedSaveSetting(key: string, value: string, delay = 300): void {
    const existing = debounceTimers.get(key);
    if (existing) clearTimeout(existing);
    debounceTimers.set(key, setTimeout(() => {
        debounceTimers.delete(key);
        saveSetting(key, value);
    }, delay));
}

/**
 * Live source of truth for the user-facing app preferences that surface in the
 * Settings dialog. Populated once at startup from `loadAllSettings()` (see
 * `initAppSettings` in `+layout.svelte`) and mutated through the setter helpers
 * below, each of which persists the change via `saveSetting`. `lastActiveFile`
 * is deliberately excluded here — it's internal bookkeeping written directly by
 * the editor, not a user-editable preference.
 */
export const appSettingsState = $state<{
    startupBehavior: StartupBehavior;
    defaultIndentMode: DefaultIndentMode;
    defaultIndentSize: number;
    confirmBeforeDelete: boolean;
}>({
    startupBehavior: DEFAULT_STARTUP_BEHAVIOR,
    defaultIndentMode: DEFAULT_DEFAULT_INDENT_MODE,
    defaultIndentSize: DEFAULT_DEFAULT_INDENT_SIZE,
    confirmBeforeDelete: DEFAULT_CONFIRM_BEFORE_DELETE,
});

/** Copy the loaded settings into the live reactive state at startup. */
export function hydrateAppSettingsState(settings: AppSettings): void {
    appSettingsState.startupBehavior = settings.startupBehavior;
    appSettingsState.defaultIndentMode = settings.defaultIndentMode;
    appSettingsState.defaultIndentSize = settings.defaultIndentSize;
    appSettingsState.confirmBeforeDelete = settings.confirmBeforeDelete;
}

export function setStartupBehavior(behavior: StartupBehavior): void {
    appSettingsState.startupBehavior = behavior;
    saveSetting(KEY_STARTUP_BEHAVIOR, behavior);
}

export function setDefaultIndentMode(mode: DefaultIndentMode): void {
    appSettingsState.defaultIndentMode = mode;
    saveSetting(KEY_DEFAULT_INDENT_MODE, mode);
}

export function setDefaultIndentSize(size: number): void {
    // Clamp to the same 1-8 range the backend enforces so the UI can't push an
    // invalid value that the command would reject.
    const clamped = Math.min(8, Math.max(1, Math.round(size)));
    appSettingsState.defaultIndentSize = clamped;
    saveSetting(KEY_DEFAULT_INDENT_SIZE, String(clamped));
}

export function setConfirmBeforeDelete(confirm: boolean): void {
    appSettingsState.confirmBeforeDelete = confirm;
    saveSetting(KEY_CONFIRM_BEFORE_DELETE, String(confirm));
}

/**
 * Persist (or clear) the last-active saved-file pointer used by the "reopen
 * last file" startup behavior. Pass `null` to clear it (e.g. when the user
 * starts a fresh untitled slate). Fire-and-forget — startup restoration is a
 * best-effort convenience, not a correctness guarantee.
 */
export function saveLastActiveDocument(
    document: { documentId: string; generation: number } | null,
): void {
    void invoke("set_last_active_document", {
        documentId: document?.documentId ?? null,
        documentGeneration: document?.generation ?? null,
    });
}
