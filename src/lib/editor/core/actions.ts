import type { EditorView } from "codemirror";
import { undo, redo, selectAll } from "@codemirror/commands";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "svelte-sonner";
import { findNext, findPrevious, replaceNext, replaceAll, SearchQuery, setSearchQuery, getSearchQuery } from "@codemirror/search";
import { editorState } from "$lib/state/editor.svelte";

export async function editorCut(view: EditorView | undefined) {
    if (!view) return;
    const selection = view.state.selection.main;
    if (selection.empty) return;
    const text = view.state.sliceDoc(selection.from, selection.to);
    try {
        await writeText(text);
        view.dispatch({
            changes: { from: selection.from, to: selection.to, insert: "" },
            userEvent: "delete.cut",
        });
        view.focus();
    } catch {
        toast.error("Failed to cut text");
    }
}

export async function editorCopy(view: EditorView | undefined) {
    if (!view) return;
    const selection = view.state.selection.main;
    if (selection.empty) return;
    const text = view.state.sliceDoc(selection.from, selection.to);
    try {
        await writeText(text);
        view.focus();
    } catch {
        toast.error("Failed to copy text");
    }
}

export async function editorPaste(view: EditorView | undefined) {
    if (!view) return;
    try {
        const text = await readText();
        if (text == null) return;
        const selection = view.state.selection.main;
        const insertPos = selection.from;
        view.dispatch({
            changes: {
                from: insertPos,
                to: selection.to,
                insert: text,
            },
            selection: { anchor: insertPos + text.length },
            scrollIntoView: true,
            userEvent: "input.paste",
        });
        view.focus();
    } catch {
        toast.error("Clipboard permission denied or failed to paste");
    }
}

export function editorSelectAll(view: EditorView | undefined) {
    if (!view) return;
    selectAll(view);
    view.focus();
}

export function editorUndo(view: EditorView | undefined) {
    if (!view) return;
    undo(view);
    view.focus();
}

export function editorRedo(view: EditorView | undefined) {
    if (!view) return;
    redo(view);
    view.focus();
}

function withView(
    view: EditorView | undefined,
    fn: (v: EditorView) => void,
    focusView: boolean = true,
) {
    if (!view) return;
    fn(view);
    if (focusView) view.focus();
}

export const editorFindNext = (v: EditorView | undefined, f = true) => withView(v, findNext, f);
export const editorFindPrevious = (v: EditorView | undefined, f = true) => withView(v, findPrevious, f);
export const editorReplaceNext = (v: EditorView | undefined, f = true) => withView(v, replaceNext, f);
export const editorReplaceAll = (v: EditorView | undefined, f = true) => withView(v, replaceAll, f);

export function editorSetSearchQuery(
    view: EditorView | undefined,
    search: string,
    replace: string = "",
    caseSensitive: boolean = false,
) {
    if (!view) return;
    view.dispatch({
        effects: setSearchQuery.of(
            new SearchQuery({ search, replace, caseSensitive, literal: true })
        )
    });
}

export function updateSearchStats(view: EditorView | undefined) {
    if (!view || !editorState.findReplace.visible || !editorState.findReplace.findText) {
        editorState.findReplace.matchCount = 0;
        editorState.findReplace.currentMatch = 0;
        return;
    }

    const query = getSearchQuery(view.state);
    if (!query || !query.valid) {
        editorState.findReplace.matchCount = 0;
        editorState.findReplace.currentMatch = 0;
        return;
    }

    let matchCount = 0;
    let currentMatch = 0;
    const cursor = query.getCursor(view.state);
    const head = view.state.selection.main.head;
    const anchor = view.state.selection.main.anchor;

    // Find min and max of selection to check if a match intersects with or is near the selection
    const selectionFrom = Math.min(head, anchor);
    const selectionTo = Math.max(head, anchor);

    // Track the first match that is strictly after the cursor, to fall back on
    // if the cursor isn't inside any match.
    let firstMatchAfter = 0;
    let hasFoundCurrent = false;

    let matchItem = cursor.next();
    while (!matchItem.done) {
        matchCount++;
        const match = matchItem.value;

        // If the match overlaps or touches the current selection, we consider it the current match
        if (!hasFoundCurrent && match.from <= selectionTo && match.to >= selectionFrom) {
            currentMatch = matchCount;
            hasFoundCurrent = true;
        } else if (!hasFoundCurrent && match.from > selectionTo && firstMatchAfter === 0) {
            firstMatchAfter = matchCount;
        }
        matchItem = cursor.next();
    }

    if (!hasFoundCurrent && matchCount > 0) {
        // If cursor is before all matches, first match is 1 (which `firstMatchAfter` would be)
        // If cursor is after all matches, `firstMatchAfter` is 0, so we wrap around to 1.
        currentMatch = firstMatchAfter || 1;
    }

    editorState.findReplace.matchCount = matchCount;
    editorState.findReplace.currentMatch = currentMatch;
}
