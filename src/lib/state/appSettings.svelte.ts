import { invoke } from "@tauri-apps/api/core";

const KEY_THEME = "theme";
const KEY_FONT_SIZE = "font_size";
const KEY_WORD_WRAP = "word_wrap";
const KEY_SIDEBAR_WIDTH = "sidebar_width";
const KEY_SIDEBAR_OPEN = "sidebar_open";

export type ThemeSetting = "dark" | "light";

export interface AppSettings {
    theme: ThemeSetting;
    fontSize: number;
    wordWrap: boolean;
    sidebarWidth: number;
    sidebarOpen: boolean;
}

export const DEFAULT_THEME: ThemeSetting = "dark";
export const DEFAULT_FONT_SIZE = 15;
export const DEFAULT_WORD_WRAP = false;
export const DEFAULT_SIDEBAR_WIDTH = 20;
export const DEFAULT_SIDEBAR_OPEN = false;

export const DEFAULT_SETTINGS: AppSettings = {
    theme: DEFAULT_THEME,
    fontSize: DEFAULT_FONT_SIZE,
    wordWrap: DEFAULT_WORD_WRAP,
    sidebarWidth: DEFAULT_SIDEBAR_WIDTH,
    sidebarOpen: DEFAULT_SIDEBAR_OPEN,
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
    return {
        theme,
        fontSize: raw[KEY_FONT_SIZE] ? parseInt(raw[KEY_FONT_SIZE], 10) : DEFAULT_FONT_SIZE,
        wordWrap: raw[KEY_WORD_WRAP] === "true",
        sidebarWidth: raw[KEY_SIDEBAR_WIDTH] ? parseInt(raw[KEY_SIDEBAR_WIDTH], 10) : DEFAULT_SIDEBAR_WIDTH,
        sidebarOpen: raw[KEY_SIDEBAR_OPEN] === "true",
    };
}

export async function saveSetting(key: string, value: string): Promise<void> {
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
