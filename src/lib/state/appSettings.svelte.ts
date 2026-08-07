import { invoke } from "$lib/ipc";
import { beginTrackedWork } from "virtual:grayslate-e2e-runtime";

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
const KEY_DEFAULT_LINE_ENDING = "default_line_ending";
const KEY_DEFAULT_ENCODING = "default_encoding";

export type ThemeSetting = "dark" | "light";
export type StartupBehavior = "new" | "last";
export type DefaultIndentMode = "spaces" | "tab";

/// A concrete line ending used by documents and the app-wide default.
export type Eol = "lf" | "crlf";

/// The app-wide default applied to brand-new documents.
export type DefaultLineEnding = Eol;
export type CharacterEncoding =
    | "utf-8"
    | "utf-8-bom"
    | "utf-16le"
    | "utf-16be"
    | "windows-1252";
export type DefaultCharacterEncoding = CharacterEncoding;

export const CHARACTER_ENCODING_OPTIONS: ReadonlyArray<{
    value: CharacterEncoding;
    label: string;
    shortLabel: string;
}> = [
    { value: "utf-8", label: "UTF-8", shortLabel: "UTF-8" },
    { value: "utf-8-bom", label: "UTF-8 with BOM", shortLabel: "UTF-8 BOM" },
    { value: "utf-16le", label: "UTF-16 LE", shortLabel: "UTF-16 LE" },
    { value: "utf-16be", label: "UTF-16 BE", shortLabel: "UTF-16 BE" },
    { value: "windows-1252", label: "Windows-1252", shortLabel: "Windows 1252" },
];

export function isCharacterEncoding(value: unknown): value is CharacterEncoding {
    return typeof value === "string" &&
        CHARACTER_ENCODING_OPTIONS.some((option) => option.value === value);
}

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
    defaultLineEnding: DefaultLineEnding;
    defaultEncoding: DefaultCharacterEncoding;
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
// LF and UTF-8 are the recommended fresh-install defaults and fail-safe
// frontend fallbacks if settings cannot be loaded.
export const DEFAULT_DEFAULT_LINE_ENDING: DefaultLineEnding = "lf";
export const DEFAULT_DEFAULT_ENCODING: DefaultCharacterEncoding = "utf-8";

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
    defaultLineEnding: DEFAULT_DEFAULT_LINE_ENDING,
    defaultEncoding: DEFAULT_DEFAULT_ENCODING,
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
    const storedLineEnding = raw[KEY_DEFAULT_LINE_ENDING];
    const storedEncoding = raw[KEY_DEFAULT_ENCODING];
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
        defaultLineEnding: storedLineEnding === "crlf" ? "crlf" : DEFAULT_DEFAULT_LINE_ENDING,
        defaultEncoding: isCharacterEncoding(storedEncoding)
            ? storedEncoding
            : DEFAULT_DEFAULT_ENCODING,
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

interface DebouncedSettingWrite {
    timer: ReturnType<typeof setTimeout>;
    finishTracking: () => void;
}

const debounceTimers = new Map<string, DebouncedSettingWrite>();

export function debouncedSaveSetting(key: string, value: string, delay = 300): void {
    const existing = debounceTimers.get(key);
    if (existing) {
        clearTimeout(existing.timer);
        existing.finishTracking();
    }

    const finishTracking = beginTrackedWork(`settings-debounce:${key}`);
    const timer = setTimeout(() => {
        debounceTimers.delete(key);
        // Keep the task pending until the persisted write finishes. The invoke
        // tracker overlaps this promise, so there is no zero-work gap between
        // the debounce firing and Rust committing the setting.
        void saveSetting(key, value).finally(finishTracking);
    }, delay);
    debounceTimers.set(key, { timer, finishTracking });
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
    defaultLineEnding: DefaultLineEnding;
    defaultEncoding: DefaultCharacterEncoding;
}>({
    startupBehavior: DEFAULT_STARTUP_BEHAVIOR,
    defaultIndentMode: DEFAULT_DEFAULT_INDENT_MODE,
    defaultIndentSize: DEFAULT_DEFAULT_INDENT_SIZE,
    confirmBeforeDelete: DEFAULT_CONFIRM_BEFORE_DELETE,
    defaultLineEnding: DEFAULT_DEFAULT_LINE_ENDING,
    defaultEncoding: DEFAULT_DEFAULT_ENCODING,
});

/** Copy the loaded settings into the live reactive state at startup. */
export function hydrateAppSettingsState(settings: AppSettings): void {
    appSettingsState.startupBehavior = settings.startupBehavior;
    appSettingsState.defaultIndentMode = settings.defaultIndentMode;
    appSettingsState.defaultIndentSize = settings.defaultIndentSize;
    appSettingsState.confirmBeforeDelete = settings.confirmBeforeDelete;
    appSettingsState.defaultLineEnding = settings.defaultLineEnding;
    appSettingsState.defaultEncoding = settings.defaultEncoding;
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

export function setDefaultLineEnding(lineEnding: DefaultLineEnding): void {
    appSettingsState.defaultLineEnding = lineEnding;
    saveSetting(KEY_DEFAULT_LINE_ENDING, lineEnding);
}

export function setDefaultEncoding(encoding: DefaultCharacterEncoding): void {
    appSettingsState.defaultEncoding = encoding;
    saveSetting(KEY_DEFAULT_ENCODING, encoding);
}

/**
 * Return the concrete EOL a new document gets. Takes the value explicitly so
 * callers that just loaded settings need not wait for reactive hydration.
 */
export function resolveLineEnding(setting: DefaultLineEnding): Eol {
    return setting;
}

/**
 * The concrete EOL a new document gets, from the live settings state.
 *
 * Brand-new documents snapshot this once at creation rather than tracking the
 * setting live, so the status bar always shows a real line ending — the same
 * model VS Code, Sublime, and Notepad++ use. Existing files never consult it:
 * their line ending is whatever was detected on open.
 *
 * Synchronous because it seeds `$state` during component initialization.
 * `EditorWrapper` re-seeds the first blank slate after persisted settings load.
 */
export function resolveDefaultEol(): Eol {
    return appSettingsState.defaultLineEnding;
}

export function resolveDefaultEncoding(): CharacterEncoding {
    return appSettingsState.defaultEncoding;
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
