import { Text } from "@codemirror/state";
import { SearchQuery } from "@codemirror/search";
import type {
    FindStatsWorkerRequest,
    FindStatsWorkerResponse,
} from "./findStatsProtocol";

const SEARCH_MATCH_CACHE_LIMIT = 20_000;
const SEARCH_CHECKPOINT_INTERVAL = 512;
const MAX_SEARCH_SCAN_MS = 500;

type SearchMatchRange = {
    from: number;
    to: number;
};

type SearchCheckpoint = {
    from: number;
    index: number;
};

type SearchStatsCache = {
    doc: Text;
    query: SearchQuery;
    matchCount: number;
    matches?: SearchMatchRange[];
    checkpoints: SearchCheckpoint[];
    approximate: boolean;
};

let searchStatsCache: SearchStatsCache | undefined;

function textToDoc(text: string): Text {
    return Text.of(text.split(/\r\n?|\n/));
}

function rebuildSearchStatsCache(text: string, query: SearchQuery): SearchStatsCache {
    const doc = textToDoc(text);
    let matchCount = 0;
    const matches: SearchMatchRange[] = [];
    const checkpoints: SearchCheckpoint[] = [];
    const scanStart = performance.now();
    let approximate = false;

    const cursor = query.getCursor(doc);
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

        if (matchCount % 1000 === 0 && performance.now() - scanStart > MAX_SEARCH_SCAN_MS) {
            approximate = true;
            break;
        }

        matchItem = cursor.next();
    }

    const nextCache: SearchStatsCache = {
        doc,
        query,
        matchCount,
        checkpoints,
        approximate,
    };

    if (!approximate && matchCount <= SEARCH_MATCH_CACHE_LIMIT) {
        nextCache.matches = matches;
    }

    searchStatsCache = nextCache;
    return nextCache;
}

function findFirstMatchAfterSelection(matches: SearchMatchRange[], selectionTo: number): number {
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
    const cursor = cache.query.getCursor(cache.doc, startFrom);
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

function getCurrentMatch(
    cache: SearchStatsCache,
    selectionFrom: number,
    selectionTo: number,
): number {
    if (cache.approximate) {
        return 0;
    }

    return cache.matches
        ? getCurrentMatchFromExactCache(cache.matches, selectionFrom, selectionTo)
        : getCurrentMatchFromCheckpoints(cache, selectionFrom, selectionTo);
}

function postResponse(response: FindStatsWorkerResponse): void {
    self.postMessage(response);
}

self.onmessage = (event: MessageEvent<FindStatsWorkerRequest>) => {
    const message = event.data;

    try {
        switch (message.type) {
            case "scan": {
                const query = new SearchQuery({
                    search: message.search,
                    caseSensitive: message.caseSensitive,
                    literal: message.literal,
                    regexp: message.regexp,
                    wholeWord: message.wholeWord,
                });

                if (!query.valid || !message.search) {
                    searchStatsCache = undefined;
                    postResponse({
                        type: "result",
                        requestId: message.requestId,
                        matchCount: 0,
                        currentMatch: 0,
                        approximate: false,
                    });
                    return;
                }

                const cache = rebuildSearchStatsCache(message.text, query);
                postResponse({
                    type: "result",
                    requestId: message.requestId,
                    matchCount: cache.matchCount,
                    currentMatch: getCurrentMatch(
                        cache,
                        message.selectionFrom,
                        message.selectionTo,
                    ),
                    approximate: cache.approximate,
                });
                return;
            }

            case "selection": {
                const cache = searchStatsCache;
                if (!cache) {
                    postResponse({
                        type: "result",
                        requestId: message.requestId,
                        matchCount: 0,
                        currentMatch: 0,
                        approximate: false,
                    });
                    return;
                }

                postResponse({
                    type: "result",
                    requestId: message.requestId,
                    matchCount: cache.matchCount,
                    currentMatch: getCurrentMatch(
                        cache,
                        message.selectionFrom,
                        message.selectionTo,
                    ),
                    approximate: cache.approximate,
                });
                return;
            }
        }
    } catch (error) {
        postResponse({
            type: "error",
            requestId: message.requestId,
            error: error instanceof Error ? error.message : "Find stats worker error",
        });
    }
};
