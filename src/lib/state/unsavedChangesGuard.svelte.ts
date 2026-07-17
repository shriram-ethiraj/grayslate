import { editorState } from "$lib/state/editor.svelte";
import { appDialogsState, openUnsavedChangesDialog, closeAppDialog } from "$lib/state/appDialogs.svelte";
import type { UnsavedChangesChoice } from "$lib/state/appDialogs.svelte";

/**
 * Central guard for actions that would move the user away from the current
 * local file while it has unsaved changes. Returns `true` when it is safe to
 * proceed (no unsaved changes, or user chose Save/Discard) and `false` when
 * the user cancelled the action.
 *
 * The dialog itself only collects the user's choice; this guard orchestrates
 * the actual save so the dialog stays reusable.
 */
export async function confirmBeforeLeavingDocument(): Promise<boolean> {
    // A document switch must not replace the active authorization while a
    // save is still completing. Reusing the shared callback waits for the
    // current action and folds the latest content into its pending save slot.
    if (editorState.saveInProgress) {
        const saveInProgress = editorState.requestSaveCurrentDocument;
        if (!saveInProgress || !(await saveInProgress())) {
            return false;
        }
    }

    // Slates (including untitled documents) are autosaved by the backend, so
    // only local files need an explicit unsaved-changes prompt.
    if (!editorState.isDirty || editorState.currentFileSource !== "local") {
        return true;
    }

    // Prevent re-entry if a prompt is already open.
    if (appDialogsState.active.type === "unsaved-changes") {
        return false;
    }

    const choice = await promptUnsavedChanges();

    if (choice === "cancel") {
        return false;
    }

    if (choice === "discard") {
        return true;
    }

    // choice === "save"
    const save = editorState.requestSaveCurrentDocument;
    if (!save) {
        return false;
    }

    const saved = await save();
    return saved;
}

function promptUnsavedChanges(): Promise<UnsavedChangesChoice> {
    return new Promise((resolve) => {
        openUnsavedChangesDialog((choice) => {
            closeAppDialog();
            resolve(choice);
        });
    });
}
