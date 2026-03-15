import type { EditorView } from "codemirror";
import { undo, redo, selectAll } from "@codemirror/commands";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "$lib/components/ui/sonner";
import { findNext, findPrevious, replaceNext, replaceAll, SearchQuery, setSearchQuery, getSearchQuery } from "@codemirror/search";
import { editorState } from "$lib/state/editor.svelte";
import type {
    FindStatsWorkerRequest,
    FindStatsWorkerResponse,
} from "../workers/findStatsProtocol";

type UpdateSearchStatsOptions = {
    docChanged?: boolean;
    forceRescan?: boolean;
};

type SearchStatsWorkerState = {
    docLength: number;
    queryKey: string;
};

let searchStatsWorker: Worker | undefined;
let searchStatsWorkerState: SearchStatsWorkerState | undefined;
let nextSearchStatsRequestId = 0;
let latestSearchStatsRequestId = 0;

function clearSearchStats(): void {
    editorState.findReplace.matchCount = 0;
    editorState.findReplace.currentMatch = 0;
    editorState.findReplace.searching = false;
}

export function clearSearchStatsCache(): void {
    searchStatsWorkerState = undefined;
    latestSearchStatsRequestId = 0;
    editorState.findReplace.searching = false;
    if (searchStatsWorker) {
        searchStatsWorker.onmessage = null;
        searchStatsWorker.terminate();
        searchStatsWorker = undefined;
    }
}

function buildSearchQueryKey(query: SearchQuery): string {
    return JSON.stringify({
        search: query.search,
        caseSensitive: query.caseSensitive,
        literal: query.literal,
        regexp: query.regexp,
        wholeWord: query.wholeWord,
    });
}

function ensureSearchStatsWorker(): Worker {
    if (!searchStatsWorker) {
        searchStatsWorker = new Worker(
            new URL("../workers/findStats.worker.ts", import.meta.url),
            { type: "module" },
        );
        searchStatsWorker.onmessage = (event: MessageEvent<FindStatsWorkerResponse>) => {
            const message = event.data;
            if (message.requestId !== latestSearchStatsRequestId) {
                return;
            }

            if (message.type === "error") {
                editorState.findReplace.searching = false;
                console.error("Find stats worker error:", message.error);
                return;
            }

            editorState.findReplace.matchCount = message.matchCount;
            editorState.findReplace.currentMatch = message.currentMatch;
            editorState.findReplace.searching = false;
        };
    }

    return searchStatsWorker;
}

function postSearchStatsRequest(request: FindStatsWorkerRequest): void {
    latestSearchStatsRequestId = request.requestId;
    ensureSearchStatsWorker().postMessage(request);
}

function nextRequestId(): number {
    nextSearchStatsRequestId += 1;
    return nextSearchStatsRequestId;
}

export function editorHasSelection(view: EditorView | undefined) {
    if (!view) return false;
    return !view.state.selection.main.empty;
}

export function getEditorAllText(view: EditorView | undefined) {
    if (!view) return "";
    return view.state.doc.toString();
}

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

export async function editorCopy(view: EditorView | undefined): Promise<boolean> {
    if (!view) return false;
    const selection = view.state.selection.main;
    if (selection.empty) return false;
    const text = view.state.sliceDoc(selection.from, selection.to);
    try {
        await writeText(text);
        view.focus();
        return true;
    } catch {
        toast.error("Failed to copy text");
        return false;
    }
}

export async function editorCopyAll(view: EditorView | undefined): Promise<boolean> {
    if (!view) return false;
    const text = getEditorAllText(view);
    if (!text) return false;
    try {
        await writeText(text);
        view.focus();
        return true;
    } catch {
        toast.error("Failed to copy text");
        return false;
    }
}

export async function editorCopySelectionOrAll(view: EditorView | undefined): Promise<boolean> {
    if (!view) return false;
    if (editorHasSelection(view)) {
        return editorCopy(view);
    }
    return editorCopyAll(view);
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

export function editorUndo(view: EditorView | undefined, focusView: boolean = true) {
    if (!view) return;
    undo(view);
    if (focusView) {
        view.focus();
    }
}

export function editorRedo(view: EditorView | undefined, focusView: boolean = true) {
    if (!view) return;
    redo(view);
    if (focusView) {
        view.focus();
    }
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

export function editorGoToLine(
    view: EditorView | undefined,
    lineNumber: number,
    focusView: boolean = true,
): boolean {
    if (!view || !Number.isFinite(lineNumber)) {
        return false;
    }

    const targetLine = Math.max(1, Math.min(view.state.doc.lines, Math.trunc(lineNumber)));
    const lineInfo = view.state.doc.line(targetLine);

    view.dispatch({
        selection: { anchor: lineInfo.from },
        scrollIntoView: true,
        userEvent: "select.goToLine",
    });

    if (focusView) {
        view.focus();
    }

    return true;
}

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
    // The CM updateListener only fires syncBindings for selectionSet or
    // docChanged — a pure search-query effect triggers neither.  Invoke
    // stats recomputation explicitly so the worker picks up the new query.
    updateSearchStats(view);
}

export function updateSearchStats(
    view: EditorView | undefined,
    options?: UpdateSearchStatsOptions,
) {
    if (!view || !editorState.findReplace.visible || !editorState.findReplace.findText) {
        clearSearchStats();
        clearSearchStatsCache();
        return;
    }

    const query = getSearchQuery(view.state);
    if (!query || !query.valid) {
        clearSearchStats();
        clearSearchStatsCache();
        return;
    }

    const head = view.state.selection.main.head;
    const anchor = view.state.selection.main.anchor;
    const selectionFrom = Math.min(head, anchor);
    const selectionTo = Math.max(head, anchor);
    const queryKey = buildSearchQueryKey(query);
    const shouldRescan =
        options?.forceRescan ||
        options?.docChanged ||
        !searchStatsWorkerState ||
        searchStatsWorkerState.docLength !== view.state.doc.length ||
        searchStatsWorkerState.queryKey !== queryKey;

    editorState.findReplace.searching = true;

    if (shouldRescan) {
        searchStatsWorkerState = {
            docLength: view.state.doc.length,
            queryKey,
        };
        postSearchStatsRequest({
            type: "scan",
            requestId: nextRequestId(),
            text: view.state.doc.toString(),
            search: query.search,
            caseSensitive: query.caseSensitive,
            literal: query.literal,
            regexp: query.regexp,
            wholeWord: query.wholeWord,
            selectionFrom,
            selectionTo,
        });
        return;
    }

    postSearchStatsRequest({
        type: "selection",
        requestId: nextRequestId(),
        selectionFrom,
        selectionTo,
    });
}
