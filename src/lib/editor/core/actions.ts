import type { EditorView } from "codemirror";
import { undo, redo, selectAll } from "@codemirror/commands";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "$lib/components/ui/sonner";
import { copyEditorRangeToClipboard } from "$lib/clipboard";
import { findNext, findPrevious, replaceNext, replaceAll, SearchQuery, setSearchQuery, getSearchQuery } from "@codemirror/search";
import { editorState } from "$lib/state/editor.svelte";
import { invoke } from "$lib/ipc";

type EditorFindResponse = {
    requestId: number;
    matchCount: number;
    currentMatch: number;
    approximate: boolean;
};

export type EditorFindOptions = {
    caseSensitive: boolean;
    wholeWord: boolean;
    useRegex: boolean;
};

type UpdateSearchStatsOptions = {
    docChanged?: boolean;
    forceRescan?: boolean;
};

type SearchStatsWorkerState = {
    docLength: number;
    queryKey: string;
};

let searchStatsState: SearchStatsWorkerState | undefined;
let nextSearchStatsRequestId = 0;
let latestSearchStatsRequestId = 0;
let scanInFlightId = 0;

function clearSearchStats(): void {
    editorState.findReplace.matchCount = 0;
    editorState.findReplace.currentMatch = 0;
    editorState.findReplace.searching = false;
    editorState.findReplace.searchError = "";
}

export function clearSearchStatsCache(): void {
    searchStatsState = undefined;
    latestSearchStatsRequestId = 0;
    scanInFlightId = 0;
    editorState.findReplace.searching = false;
    editorState.findReplace.searchError = "";
    invoke("cancel_editor_find").catch(() => {});
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
    return copyEditorRangeToClipboard(view, selection.from, selection.to);
}

export async function editorCopyAll(view: EditorView | undefined): Promise<boolean> {
    if (!view) return false;
    return copyEditorRangeToClipboard(view, 0, view.state.doc.length);
}

export async function editorCopySelectionOrAll(view: EditorView | undefined): Promise<boolean> {
    if (!view) return false;
    if (editorHasSelection(view)) {
        return editorCopy(view);
    }
    return editorCopyAll(view);
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
    options?: EditorFindOptions,
) {
    if (!view) return;
    const caseSensitive = options?.caseSensitive ?? false;
    const wholeWord = options?.wholeWord ?? false;
    const useRegex = options?.useRegex ?? false;
    view.dispatch({
        effects: setSearchQuery.of(
            new SearchQuery({
                search,
                replace,
                caseSensitive,
                literal: !useRegex,
                regexp: useRegex,
                wholeWord,
            })
        )
    });
    // The CM updateListener only fires syncBindings for selectionSet or
    // docChanged — a pure search-query effect triggers neither.  Invoke
    // stats recomputation explicitly so the backend picks up the new query.
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
        !searchStatsState ||
        searchStatsState.docLength !== view.state.doc.length ||
        searchStatsState.queryKey !== queryKey;

    editorState.findReplace.searching = true;

    if (shouldRescan) {
        searchStatsState = {
            docLength: view.state.doc.length,
            queryKey,
        };
        const requestId = nextRequestId();
        latestSearchStatsRequestId = requestId;
        scanInFlightId = requestId;
        editorState.findReplace.searchError = "";
        performScan(requestId, view, query, selectionFrom, selectionTo);
        return;
    }

    // Skip selection-only updates while a scan is in flight —
    // the scan result will include the correct currentMatch.
    if (scanInFlightId > 0) return;

    const requestId = nextRequestId();
    latestSearchStatsRequestId = requestId;
    performSelectionUpdate(requestId, selectionFrom, selectionTo);
}

async function performScan(
    requestId: number,
    view: EditorView,
    query: SearchQuery,
    selectionFrom: number,
    selectionTo: number,
): Promise<void> {
    try {
        const result = await invoke<EditorFindResponse>("editor_find_scan", {
            text: view.state.doc.toString(),
            search: query.search,
            caseSensitive: query.caseSensitive,
            wholeWord: query.wholeWord,
            useRegex: !query.literal,
            selectionFrom,
            selectionTo,
            requestId,
        });
        if (result.requestId !== latestSearchStatsRequestId) return;
        editorState.findReplace.matchCount = result.matchCount;
        editorState.findReplace.currentMatch = result.currentMatch;
        editorState.findReplace.searching = false;
        editorState.findReplace.searchError = "";
    } catch (e) {
        if (requestId !== latestSearchStatsRequestId) return;
        if (typeof e === "string" && e === "Cancelled") return;
        editorState.findReplace.searching = false;
        if (typeof e === "string" && e.startsWith("Invalid regex")) {
            editorState.findReplace.searchError = e;
            editorState.findReplace.matchCount = 0;
            editorState.findReplace.currentMatch = 0;
        } else {
            editorState.findReplace.searchError = "";
            console.error("Editor find scan error:", e);
        }
    } finally {
        if (scanInFlightId === requestId) {
            scanInFlightId = 0;
        }
    }
}

async function performSelectionUpdate(
    requestId: number,
    selectionFrom: number,
    selectionTo: number,
): Promise<void> {
    try {
        const result = await invoke<EditorFindResponse>("editor_find_selection", {
            selectionFrom,
            selectionTo,
            requestId,
        });
        if (result.requestId !== latestSearchStatsRequestId) return;
        editorState.findReplace.matchCount = result.matchCount;
        editorState.findReplace.currentMatch = result.currentMatch;
        editorState.findReplace.searching = false;
    } catch {
        if (requestId !== latestSearchStatsRequestId) return;
        editorState.findReplace.searching = false;
    }
}
