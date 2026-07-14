import type { AppOs } from "$lib/state/platform.svelte";

/** Platform names accepted by TanStack Hotkeys' display formatter. */
export type ShortcutPlatform = "mac" | "windows" | "linux";

/**
 * A display key uses the same notation as the shared TanStack hotkey
 * registrations.  It intentionally remains a string because a few valid
 * navigation combinations (for example Mod+Home) are outside the library's
 * narrow RegisterableHotkey union.
 */
export type ShortcutKey = string;

export type ShortcutDefinition = {
    id: string;
    label: string;
    keys: readonly ShortcutKey[];
};

export type ShortcutCategory = {
    id: string;
    label: string;
    shortcuts: readonly ShortcutDefinition[];
};

export const shortcutCategories = [
    {
        id: "general",
        label: "General",
        shortcuts: [
            { id: "new-slate", label: "New Slate", keys: ["Mod+N"] },
            { id: "open-file", label: "Open File", keys: ["Mod+O"] },
            { id: "save-file", label: "Save", keys: ["Mod+S"] },
            { id: "save-file-as", label: "Save As", keys: ["Mod+Shift+S"] },
            { id: "settings", label: "Settings", keys: ["Mod+,"] },
        ],
    },
    {
        id: "navigation",
        label: "Navigation",
        shortcuts: [
            { id: "toggle-sidebar", label: "Toggle Sidebar", keys: ["Mod+B"] },
            { id: "find-files", label: "Find Files", keys: ["Mod+P"] },
            { id: "go-to-line", label: "Go To Line", keys: ["Mod+G"] },
            { id: "sidebar-previous", label: "Previous Sidebar Result", keys: ["ArrowUp"] },
            { id: "sidebar-next", label: "Next Sidebar Result", keys: ["ArrowDown"] },
            { id: "sidebar-open", label: "Open Highlighted Sidebar Result", keys: ["Enter"] },
        ],
    },
    {
        id: "editor",
        label: "Editor",
        shortcuts: [
            { id: "undo", label: "Undo", keys: ["Mod+Z"] },
            { id: "redo", label: "Redo", keys: ["Mod+Shift+Z", "Mod+Y"] },
            { id: "cut", label: "Cut", keys: ["Mod+X"] },
            { id: "copy", label: "Copy", keys: ["Mod+C"] },
            { id: "paste", label: "Paste", keys: ["Mod+V"] },
            { id: "select-all", label: "Select All", keys: ["Mod+A"] },
            { id: "find", label: "Find", keys: ["Mod+F"] },
            { id: "replace", label: "Replace", keys: ["Mod+Alt+F", "Mod+H"] },
            { id: "word-wrap", label: "Toggle Word Wrap", keys: ["Alt+Z"] },
            {
                id: "transformations",
                label: "Open Transformations",
                keys: ["Mod+K", "Mod+Shift+P"],
            },
            { id: "indent", label: "Indent Selection", keys: ["Tab"] },
            { id: "outdent", label: "Outdent Selection", keys: ["Shift+Tab"] },
        ],
    },
    {
        id: "find-replace",
        label: "Find & Replace",
        shortcuts: [
            { id: "find-next", label: "Next Match", keys: ["Enter"] },
            { id: "find-previous", label: "Previous Match", keys: ["Shift+Enter"] },
            { id: "replace-next", label: "Apply Replacement", keys: ["Enter"] },
            { id: "find-replace-close", label: "Close Find & Replace", keys: ["Escape"] },
        ],
    },
    {
        id: "view",
        label: "View",
        shortcuts: [
            {
                id: "increase-font-size",
                label: "Increase Font Size",
                keys: ["Mod+=", "Mod+Shift+="],
            },
            { id: "decrease-font-size", label: "Decrease Font Size", keys: ["Mod+-"] },
            { id: "reset-font-size", label: "Reset Font Size", keys: ["Mod+0"] },
        ],
    },
    {
        id: "csv-table",
        label: "CSV Table",
        shortcuts: [
            { id: "csv-undo", label: "Undo Table Edit", keys: ["Mod+Z"] },
            { id: "csv-redo", label: "Redo Table Edit", keys: ["Mod+Shift+Z", "Mod+Y"] },
            { id: "csv-move-row-up", label: "Move Selected Rows Up", keys: ["Alt+ArrowUp"] },
            { id: "csv-move-row-down", label: "Move Selected Rows Down", keys: ["Alt+ArrowDown"] },
            { id: "csv-move-column-left", label: "Move Selected Columns Left", keys: ["Alt+ArrowLeft"] },
            { id: "csv-move-column-right", label: "Move Selected Columns Right", keys: ["Alt+ArrowRight"] },
            { id: "csv-insert-row-above", label: "Insert Row Above", keys: ["Mod+Alt+ArrowUp"] },
            { id: "csv-insert-row-below", label: "Insert Row Below", keys: ["Mod+Alt+ArrowDown"] },
            { id: "csv-insert-column-left", label: "Insert Column Left", keys: ["Mod+Alt+ArrowLeft"] },
            { id: "csv-insert-column-right", label: "Insert Column Right", keys: ["Mod+Alt+ArrowRight"] },
            { id: "csv-start-edit", label: "Edit Focused Cell", keys: ["Enter", "F2"] },
            { id: "csv-clear", label: "Clear Cell or Selection", keys: ["Delete", "Backspace"] },
            { id: "csv-move-up", label: "Move Focus Up", keys: ["ArrowUp"] },
            { id: "csv-move-down", label: "Move Focus Down", keys: ["ArrowDown"] },
            { id: "csv-move-left", label: "Move Focus Left", keys: ["ArrowLeft"] },
            { id: "csv-move-right", label: "Move Focus Right", keys: ["ArrowRight"] },
            { id: "csv-extend-up", label: "Extend Selection Up", keys: ["Shift+ArrowUp"] },
            { id: "csv-extend-down", label: "Extend Selection Down", keys: ["Shift+ArrowDown"] },
            { id: "csv-extend-left", label: "Extend Selection Left", keys: ["Shift+ArrowLeft"] },
            { id: "csv-extend-right", label: "Extend Selection Right", keys: ["Shift+ArrowRight"] },
            { id: "csv-next-cell", label: "Move to Next Cell", keys: ["Tab"] },
            { id: "csv-previous-cell", label: "Move to Previous Cell", keys: ["Shift+Tab"] },
            { id: "csv-first-column", label: "Move to First Column", keys: ["Home"] },
            { id: "csv-last-column", label: "Move to Last Column", keys: ["End"] },
            { id: "csv-first-cell", label: "Move to First Cell", keys: ["Mod+Home"] },
            { id: "csv-last-cell", label: "Move to Last Cell", keys: ["Mod+End"] },
            { id: "csv-page-up", label: "Move One Page Up", keys: ["PageUp"] },
            { id: "csv-page-down", label: "Move One Page Down", keys: ["PageDown"] },
            { id: "csv-clear-selection", label: "Clear Table Selection", keys: ["Escape"] },
        ],
    },
] as const satisfies readonly ShortcutCategory[];

export type ShortcutCategoryId = (typeof shortcutCategories)[number]["id"];

/** Convert Tauri's OS names to the names expected by formatForDisplay. */
export function getShortcutPlatform(osType: AppOs | undefined): ShortcutPlatform {
    switch (osType) {
        case "macos":
            return "mac";
        case "linux":
            return "linux";
        case "windows":
        default:
            return "windows";
    }
}
