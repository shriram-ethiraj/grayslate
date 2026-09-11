import { editorState } from "$lib/state/editor.svelte";
import { appDialogsState, openUnsavedChangesDialog, closeAppDialog } from "$lib/state/appDialogs.svelte";
import type { UnsavedChangesChoice } from "$lib/state/appDialogs.svelte";
import type { UnsavedChangesAlternative } from "$lib/state/appDialogs.svelte";

export type DocumentSwitchDecision = "open-here" | "new-window" | "cancel";

/**
 * Central guard for actions that would move the user away from the current
 * local file while it has unsaved changes. Returns `true` when it is safe to
 * proceed (no unsaved changes, or user chose Save/Discard) and `false` when
 * the user cancelled the action.
 *
 * The dialog itself only collects the user's choice; this guard orchestrates
 * the actual save so the dialog stays reusable.
 */
export async function confirmBeforeLeavingDocument(
    options: {
        /**
         * Read the editor's live document state when a caller cannot rely on
         * the debounced global dirty flag yet.
         */
        hasUnsavedLocalChanges?: () => boolean;
    } = {},
): Promise<boolean> {
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
    const hasUnsavedLocalChanges =
        options.hasUnsavedLocalChanges?.() ?? editorState.isDirty;
    if (!hasUnsavedLocalChanges || editorState.currentFileSource !== "local") {
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

    if (choice === "new-window") {
        return false;
    }

    // choice === "save"
    const save = editorState.requestSaveCurrentDocument;
    if (!save) {
        return false;
    }

    const saved = await save();
    return saved;
}

/**
 * Resolve a document-switch prompt while offering a non-destructive window
 * alternative. Saving and discarding mean "continue here"; the alternative
 * leaves the current editor and its dirty state untouched.
 */
export async function chooseDocumentSwitch(
    alternative: UnsavedChangesAlternative,
): Promise<DocumentSwitchDecision> {
    if (editorState.saveInProgress) {
        const saveInProgress = editorState.requestSaveCurrentDocument;
        if (!saveInProgress || !(await saveInProgress())) return "cancel";
    }

    if (!editorState.isDirty || editorState.currentFileSource !== "local") {
        return "open-here";
    }
    if (appDialogsState.active.type === "unsaved-changes") return "cancel";

    const choice = await promptUnsavedChanges(alternative);
    if (choice === "cancel") return "cancel";
    if (choice === "new-window") return "new-window";
    if (choice === "discard") return "open-here";

    const save = editorState.requestSaveCurrentDocument;
    return save && await save() ? "open-here" : "cancel";
}

function promptUnsavedChanges(
    alternative?: UnsavedChangesAlternative,
): Promise<UnsavedChangesChoice> {
    return new Promise((resolve) => {
        openUnsavedChangesDialog((choice) => {
            closeAppDialog();
            resolve(choice);
        }, alternative);
    });
}
