import type { RecentFileRecord } from "$lib/files/recentFiles";

export type UnsavedChangesChoice = "save" | "discard" | "cancel" | "new-window";
export type UnsavedChangesAlternative =
    | "open-in-new-window"
    | "open-all-in-new-windows"
    | "create-in-new-window";

/**
 * Discriminated union covering every app-level dialog.  Only one dialog can be
 * active at a time — the union type makes this structurally impossible to
 * violate.
 */
export type AppDialogState =
    | { type: "none" }
    | { type: "about" }
    | { type: "settings" }
    | { type: "keyboard-shortcuts" }
    | { type: "delete"; file: RecentFileRecord }
    | { type: "rename"; file: RecentFileRecord }
    | {
        type: "unsaved-changes";
        alternative?: UnsavedChangesAlternative;
        resolve: (choice: UnsavedChangesChoice) => void;
    };

export const appDialogsState = $state<{ active: AppDialogState }>({
    active: { type: "none" },
});

export function openAboutAppDialog(): void {
    appDialogsState.active = { type: "about" };
}

export function openSettingsAppDialog(): void {
    appDialogsState.active = { type: "settings" };
}

export function openKeyboardShortcutsAppDialog(): void {
    appDialogsState.active = { type: "keyboard-shortcuts" };
}

export function openDeleteFileDialog(file: RecentFileRecord): void {
    appDialogsState.active = { type: "delete", file };
}

export function openRenameFileDialog(file: RecentFileRecord): void {
    appDialogsState.active = { type: "rename", file };
}

export function openUnsavedChangesDialog(
    resolve: (choice: UnsavedChangesChoice) => void,
    alternative?: UnsavedChangesAlternative,
): void {
    appDialogsState.active = { type: "unsaved-changes", alternative, resolve };
}

export function closeAppDialog(): void {
    appDialogsState.active = { type: "none" };
}
