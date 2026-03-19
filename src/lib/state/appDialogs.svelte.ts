import type { RecentFileRecord } from "$lib/files/recentFiles";

/**
 * Discriminated union covering every app-level dialog.  Only one dialog can be
 * active at a time — the union type makes this structurally impossible to
 * violate.
 */
export type AppDialogState =
    | { type: "none" }
    | { type: "about" }
    | { type: "delete"; file: RecentFileRecord }
    | { type: "rename"; file: RecentFileRecord };

export const appDialogsState = $state<{ active: AppDialogState }>({
    active: { type: "none" },
});

export function openAboutAppDialog(): void {
    appDialogsState.active = { type: "about" };
}

export function openDeleteFileDialog(file: RecentFileRecord): void {
    appDialogsState.active = { type: "delete", file };
}

export function openRenameFileDialog(file: RecentFileRecord): void {
    appDialogsState.active = { type: "rename", file };
}

export function closeAppDialog(): void {
    appDialogsState.active = { type: "none" };
}
