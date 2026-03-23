import type { RecentFileSource } from "$lib/files/recentFiles";

export interface PendingSidebarFileOpen {
    path: string;
    source: RecentFileSource;
    requestId: number;
    revealInRecentList: boolean;
    lineNumber?: number;
}

export const librarySidebarState = $state<{
    pendingOpenFile: PendingSidebarFileOpen | undefined;
    requestActivateSearch?: () => void;
    /** Registered by the sidebar. Call after a rename to refresh metadata
     *  (new filename) without clearing suppression or reordering the list. */
    requestQuietDataRefresh?: () => void;
    /** Set by the rename dialog so the sidebar can update its suppression
     *  tracking path instead of misinterpreting the rename as a navigation. */
    lastRenamedPath?: { from: string; to: string };
}>({
    pendingOpenFile: undefined,
    requestActivateSearch: undefined,
    requestQuietDataRefresh: undefined,
    lastRenamedPath: undefined,
});

export function setPendingSidebarOpenFile(pendingOpenFile: PendingSidebarFileOpen): void {
    librarySidebarState.pendingOpenFile = pendingOpenFile;
}

export function clearPendingSidebarOpenFile(requestId?: number): void {
    if (requestId === undefined) {
        librarySidebarState.pendingOpenFile = undefined;
        return;
    }

    if (librarySidebarState.pendingOpenFile?.requestId === requestId) {
        librarySidebarState.pendingOpenFile = undefined;
    }
}