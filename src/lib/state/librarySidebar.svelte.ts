import type { RecentFileSource } from "$lib/files/recentFiles";

export interface PendingSidebarFileOpen {
    path: string;
    source: RecentFileSource;
    requestId: number;
    revealInRecentList: boolean;
    lineNumber?: number;
}

/**
 * Semantic result of a library-affecting file operation. The sidebar owns the
 * refresh and navigation policy; callers only report what changed.
 */
export type LibraryMutation =
    | { kind: "created"; path: string; source: RecentFileSource }
    | { kind: "opened"; path: string; source: RecentFileSource; origin: "sidebar" | "local" }
    | { kind: "saved" }
    | { kind: "duplicated"; path: string; source: RecentFileSource }
    | { kind: "removed"; path: string }
    | { kind: "renamed"; from: string; to: string }
    | { kind: "sync" };

export const librarySidebarState = $state<{
    pendingOpenFile: PendingSidebarFileOpen | undefined;
    requestActivateSearch?: () => void;
    /** Registered by the sidebar. Call after any library-affecting operation. */
    handleLibraryMutation?: (mutation: LibraryMutation) => void;
}>({
    pendingOpenFile: undefined,
    requestActivateSearch: undefined,
    handleLibraryMutation: undefined,
});

export function reportLibraryMutation(mutation: LibraryMutation): void {
    librarySidebarState.handleLibraryMutation?.(mutation);
}

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
