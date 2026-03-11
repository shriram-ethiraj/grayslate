import type { RecentFileSource } from "$lib/files/recentFiles";

export interface PendingSidebarFileOpen {
    path: string;
    source: RecentFileSource;
    requestId: number;
    revealInRecentList: boolean;
}

export const librarySidebarState = $state<{
    pendingOpenFile: PendingSidebarFileOpen | undefined;
}>({
    pendingOpenFile: undefined,
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