import type { EditorView } from "codemirror";
import { undo, redo, selectAll } from "@codemirror/commands";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "svelte-sonner";
import { findNext, findPrevious, replaceNext, replaceAll, SearchQuery, setSearchQuery, getSearchQuery } from "@codemirror/search";
import { editorState } from "$lib/state/editor.svelte";

const SEARCH_MATCH_CACHE_LIMIT = 20_000;
const SEARCH_CHECKPOINT_INTERVAL = 512;

type SearchMatchRange = {
    from: number;
    to: number;
};

type SearchCheckpoint = {
    from: number;
    index: number;
};

type SearchStatsCache = {
    docLength: number;
    query: SearchQuery;
    matchCount: number;
    matches?: SearchMatchRange[];
    checkpoints: SearchCheckpoint[];
};

type UpdateSearchStatsOptions = {
    docChanged?: boolean;
    forceRescan?: boolean;
};

let searchStatsCache: SearchStatsCache | undefined;

function clearSearchStats(): void {
    editorState.findReplace.matchCount = 0;
    editorState.findReplace.currentMatch = 0;
}

function clearSearchStatsCache(): void {
    searchStatsCache = undefined;
}

function shouldReuseSearchStatsCache(
    cache: SearchStatsCache | undefined,
    view: EditorView,
    query: SearchQuery,
    options?: UpdateSearchStatsOptions,
): cache is SearchStatsCache {
    if (!cache || options?.forceRescan || options?.docChanged) {
        return false;
    }

    return cache.docLength === view.state.doc.length && cache.query.eq(query);
}

function rebuildSearchStatsCache(view: EditorView, query: SearchQuery): SearchStatsCache {
    let matchCount = 0;
    const matches: SearchMatchRange[] = [];
    const checkpoints: SearchCheckpoint[] = [];

    const cursor = query.getCursor(view.state);
    let matchItem = cursor.next();
    while (!matchItem.done) {
        matchCount += 1;
        const match = matchItem.value;

        if (matches.length < SEARCH_MATCH_CACHE_LIMIT) {
            matches.push({ from: match.from, to: match.to });
        }

        if (matchCount === 1 || matchCount % SEARCH_CHECKPOINT_INTERVAL === 0) {
            checkpoints.push({ from: match.from, index: matchCount });
        }

        matchItem = cursor.next();
    }

    const nextCache: SearchStatsCache = {
        docLength: view.state.doc.length,
        query,
        matchCount,
        checkpoints,
    };

    if (matchCount <= SEARCH_MATCH_CACHE_LIMIT) {
        nextCache.matches = matches;
    }

    searchStatsCache = nextCache;
    return nextCache;
}

function findFirstMatchAfterSelection(
    matches: SearchMatchRange[],
    selectionTo: number,
): number {
    let low = 0;
    let high = matches.length;

    while (low < high) {
        const mid = Math.floor((low + high) / 2);
        if (matches[mid].from <= selectionTo) {
            low = mid + 1;
        } else {
            high = mid;
        }
    }

    return low;
}

function getCurrentMatchFromExactCache(
    matches: SearchMatchRange[],
    selectionFrom: number,
    selectionTo: number,
): number {
    if (matches.length === 0) {
        return 0;
    }

    const firstAfterIndex = findFirstMatchAfterSelection(matches, selectionTo);
    const candidateIndex = Math.max(0, firstAfterIndex - 1);
    const candidate = matches[candidateIndex];

    if (candidate && candidate.from <= selectionTo && candidate.to >= selectionFrom) {
        return candidateIndex + 1;
    }

    if (firstAfterIndex < matches.length) {
        return firstAfterIndex + 1;
    }

    return 1;
}

function getCurrentMatchFromCheckpoints(
    view: EditorView,
    query: SearchQuery,
    cache: SearchStatsCache,
    selectionFrom: number,
    selectionTo: number,
): number {
    if (cache.matchCount === 0) {
        return 0;
    }

    let checkpoint: SearchCheckpoint | undefined;
    let low = 0;
    let high = cache.checkpoints.length;

    while (low < high) {
        const mid = Math.floor((low + high) / 2);
        if (cache.checkpoints[mid].from <= selectionTo) {
            checkpoint = cache.checkpoints[mid];
            low = mid + 1;
        } else {
            high = mid;
        }
    }

    const startFrom = checkpoint?.from ?? 0;
    let currentIndex = checkpoint ? checkpoint.index - 1 : 0;
    const cursor = query.getCursor(view.state, startFrom);
    let matchItem = cursor.next();

    while (!matchItem.done) {
        currentIndex += 1;
        const match = matchItem.value;

        if (match.from <= selectionTo && match.to >= selectionFrom) {
            return currentIndex;
        }

        if (match.from > selectionTo) {
            return currentIndex;
        }

        matchItem = cursor.next();
    }

    return 1;
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

export function editorSetSearchQuery(
    view: EditorView | undefined,
    search: string,
    replace: string = "",
    caseSensitive: boolean = false,
) {
    if (!view) return;
    clearSearchStatsCache();
    view.dispatch({
        effects: setSearchQuery.of(
            new SearchQuery({ search, replace, caseSensitive, literal: true })
        )
    });
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

    const cache = shouldReuseSearchStatsCache(searchStatsCache, view, query, options)
        ? searchStatsCache
        : rebuildSearchStatsCache(view, query);

    editorState.findReplace.matchCount = cache.matchCount;
    editorState.findReplace.currentMatch = cache.matches
        ? getCurrentMatchFromExactCache(cache.matches, selectionFrom, selectionTo)
        : getCurrentMatchFromCheckpoints(view, query, cache, selectionFrom, selectionTo);
}
