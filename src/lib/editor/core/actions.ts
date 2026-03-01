import type { EditorView } from "codemirror";
import { undo, redo, selectAll } from "@codemirror/commands";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "svelte-sonner";

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
