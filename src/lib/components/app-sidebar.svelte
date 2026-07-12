<script lang="ts">
    import { registerHotkey } from "$lib/hotkeys";
    import * as Sidebar from "$lib/components/ui/sidebar/index.js";
    import { editorState } from "$lib/state/editor.svelte";
    import {
        librarySidebarState,
        type LibraryMutation,
        reportLibraryMutation,
        setPendingSidebarOpenFile,
    } from "$lib/state/librarySidebar.svelte";
    import { confirmBeforeLeavingDocument } from "$lib/state/unsavedChangesGuard.svelte";
    import {
        getRecentFiles,
        OPEN_FILE_PATH_EVENT,
        RECENT_FILES_UPDATED_EVENT,
        type OpenFilePathPayload,
        type RecentFileRecord,
        type RecentFileSource,
        type SidebarSearchResult,
        type SearchOptions,
        DEFAULT_SEARCH_OPTIONS,
        searchSidebarFiles,
        cancelSidebarSearch,
        duplicateFile,
        duplicateLocalFileAsSlate,
        performFileUnlink,
    } from "$lib/files/recentFiles";
    import {
        buildRecencySections,
        compareRecentFiles,
        compareSearchResults,
        DEFAULT_FILTER_MODE,
        DEFAULT_SORT_MODE,
        RECENT_FILES_LIMIT,
        type FilterMode,
        type LibraryFileRecord,
        type RecentFileSection,
        type SortMode,
    } from "$lib/files/sidebarUtils";
    import { toast } from "$lib/components/ui/sonner";
    import SidebarHeader from "$lib/components/sidebar/SidebarHeader.svelte";
    import SidebarFileList from "$lib/components/sidebar/SidebarFileList.svelte";
    import { useListNavigator } from "$lib/components/sidebar/useListNavigator.svelte";
    import { createLibraryRefreshCoordinator } from "$lib/files/libraryRefreshCoordinator";

    // ---------------------------------------------------------------------------
    // Component state
    // ---------------------------------------------------------------------------

    let query = $state("");
    let filterMode = $state<FilterMode>(DEFAULT_FILTER_MODE);
    let sortMode = $state<SortMode>(DEFAULT_SORT_MODE);
    let searchOptions = $state<SearchOptions>({ ...DEFAULT_SEARCH_OPTIONS });

    let recentFiles = $state<RecentFileRecord[]>([]);

    let searchResults = $state<SidebarSearchResult[]>([]);
    let isLoading = $state(false);
    let isSearchLoading = $state(false);
    let loadError = $state("");

    // ---------------------------------------------------------------------------
    // Reorder suppression policy
    //
    // Goal: the visible file list must never shift under the user's cursor
    // while they are browsing or navigating via the sidebar.
    //
    // When active (`suppressReorder = true`):
    //   • Generic RECENT_FILES_UPDATED_EVENT refreshes are silently deferred;
    //     explicit content-save events refresh immediately.
    //   • The sort is still applied so the user's selected sort mode is respected.
    //
    // Activated by:
    //   • Opening a file from the sidebar (`openRecentFile`).
    //
    // Cleared only by explicit user actions:
    //   • Changing the sort order.
    //   • Clicking the manual refresh button.
    //   • Closing the sidebar (pre-fetches invisibly on close so reopen
    //     shows fresh data without a visible transition).
    //   • An external navigation that doesn't match the sidebar-opened path
    //     (safety valve so suppression doesn't stick forever).
    //
    // NOT cleared by:
    //   • Background backend events unrelated to a content save.
    //   • Filter (tab) changes while suppression is active.
    //
    // Successful structural actions (create, rename, remove, duplicate) are
    // explicit user intent and therefore clear suppression immediately.
    // ---------------------------------------------------------------------------
    let suppressReorder = $state(false);
    // The path of the last file opened via the sidebar, used to decide whether
    // to keep suppressReorder active when the editor navigation event fires.
    let lastSidebarOpenedPath = $state<string | undefined>(undefined);

    // Search requests may overlap because query changes are debounced; stale
    // responses are discarded by this version counter.
    let searchRequestVersion = 0;

    // DOM ref for the scrollable results container (propagated from SidebarFileList).
    let resultsScrollContainer = $state<HTMLDivElement | null>(null);
    // Incrementing counter: bump to request focus of the search input in SidebarHeader.
    let focusSearchRequest = $state(0);
    // True when focus is anywhere inside the sidebar panel (input, list, buttons).
    // Use this to gate sidebar-wide keyboard shortcuts (e.g. Left/Right tab nav).
    let isSidebarFocused = $state(false);

    // Previous values used to detect *changes* in effects without triggering
    // on the initial run. Initialized to the current values so the first pass
    // is always a no-op.
    let lastObservedEditorPath: string | undefined = editorState.currentFilePath;
    let lastObservedUntitledState = editorState.isUntitledDocument;
    let lastObservedFilterMode: FilterMode = DEFAULT_FILTER_MODE;
    let lastObservedSortMode: SortMode = DEFAULT_SORT_MODE;
    let lastObservedSidebarOpen = true;

    // The path to reveal after a refresh has placed it in the current list.
    // This makes external opens and duplicates resilient to backend event timing.
    let pendingRevealPath = $state<string | undefined>(undefined);

    // ---------------------------------------------------------------------------
    // Derived state
    // ---------------------------------------------------------------------------

    const sidebar = Sidebar.useSidebar();

    const normalizedQuery = $derived(query.trim().toLowerCase());
    // Boolean derived that only changes at the empty ↔ non-empty boundary, never
    // on intermediate keystrokes. Downstream derivations that only need to know
    // "are we searching?" depend on this instead of normalizedQuery directly, so
    // they don't re-run on every character the user types.
    const isSearchMode = $derived(normalizedQuery.length > 0);
    // Stable string key that changes only when the user toggles a search option.
    // Used in the debounce $effect to re-trigger search without reading the
    // individual $state booleans (which would also work, but this is explicit).
    const searchOptionsKey = $derived(
        `${searchOptions.caseSensitive}:${searchOptions.wholeWord}:${searchOptions.useRegex}`,
    );
    const pendingOpenFile = $derived(librarySidebarState.pendingOpenFile);

    const visibleRecentFiles = $derived.by(() => {
        const filteredRecentFiles = recentFiles.filter((recentFile) =>
            filterMode === "unified" || recentFile.source === filterMode
        );

        filteredRecentFiles.sort((left, right) => compareRecentFiles(left, right, sortMode));
        return filteredRecentFiles;
    });

    const sortedSearchResults = $derived.by(() => {
        const sorted = [...searchResults];
        sorted.sort((left, right) => compareSearchResults(left, right, sortMode));
        return sorted;
    });

    const activeResults = $derived<LibraryFileRecord[]>(
        isSearchMode ? sortedSearchResults : visibleRecentFiles,
    );

    // -----------------------------------------------------------------------
    // List navigation (keyboard, hover, scroll)
    // -----------------------------------------------------------------------

    const navigator = useListNavigator({
        getActiveResults: () => activeResults,
        getScrollContainer: () => resultsScrollContainer,
        onOpen: (path, source) => openRecentFile(path, source),
    });

    const recentFileSections = $derived.by((): RecentFileSection[] => {
        if (
            isSearchMode ||
            (sortMode !== "recently-opened" && sortMode !== "least-recently-opened")
        ) {
            return [{ key: "all", label: "", items: activeResults }];
        }

        return buildRecencySections(activeResults, sortMode);
    });

    // ---------------------------------------------------------------------------
    // Reorder suppression
    // ---------------------------------------------------------------------------

    function clearReorderSuppression(): void {
        suppressReorder = false;
        lastSidebarOpenedPath = undefined;
        refreshCoordinator.releaseDeferred();
    }

    // ---------------------------------------------------------------------------
    // Data fetching
    // ---------------------------------------------------------------------------

    async function fetchRecentFiles(showLoading: boolean): Promise<void> {
        if (showLoading) {
            isLoading = true;
        }

        // Use the stable isSearchMode boolean instead of normalizedQuery so
        // callers inside $effect don't accidentally subscribe to normalizedQuery
        // (which changes on every keystroke).
        if (showLoading && !isSearchMode) {
            loadError = "";
        }

        try {
            const result = await getRecentFiles(RECENT_FILES_LIMIT);
            recentFiles = result;
        } catch (error: unknown) {
            if (showLoading && !isSearchMode) {
                loadError = typeof error === "string"
                    ? error
                    : "Failed to load recent files.";
            }
        } finally {
            if (showLoading) {
                isLoading = false;
            }
        }
    }

    async function refreshSearchResults(): Promise<void> {
        const currentVersion = ++searchRequestVersion;
        isSearchLoading = true;
        loadError = "";

        try {
            const result = await searchSidebarFiles(
                query.trim(),
                filterMode,
                currentVersion,
                searchOptions,
            );
            if (currentVersion !== searchRequestVersion) {
                return;
            }

            searchResults = result;
            navigator.resetToFile(editorState.currentFilePath);
        } catch (error: unknown) {
            if (currentVersion !== searchRequestVersion) {
                return;
            }

            const message = typeof error === "string"
                ? error
                : "Failed to search files.";
            if (message !== "Search cancelled.") {
                loadError = message;
            }
        } finally {
            if (currentVersion === searchRequestVersion) {
                isSearchLoading = false;
            }
        }
    }

    const refreshCoordinator = createLibraryRefreshCoordinator({
        isSuppressed: () => suppressReorder,
        isSearchMode: () => isSearchMode,
        refreshRecent: fetchRecentFiles,
        refreshSearch: refreshSearchResults,
    });

    // ---------------------------------------------------------------------------
    // UI actions
    // ---------------------------------------------------------------------------

    async function openRecentFile(path: string, source: RecentFileSource, lineNumber?: number): Promise<void> {
        // Already the open file — nothing to navigate away from, so don't
        // prompt for unsaved changes or re-trigger the open flow.
        if (path === editorState.currentFilePath) return;

        if (!(await confirmBeforeLeavingDocument())) return;

        // Freeze the list order so opening a file doesn't immediately re-sort
        // the sidebar, which would be jarring for sequential file navigation.
        suppressReorder = true;
        lastSidebarOpenedPath = path;
        navigator.focusHighlight(path);

        const requestId = Date.now();
        setPendingSidebarOpenFile({
            path,
            source,
            requestId,
            revealInRecentList: false,
            lineNumber,
        });

        const { emit } = await import("@tauri-apps/api/event");
        await emit(OPEN_FILE_PATH_EVENT, { path, source, lineNumber } satisfies OpenFilePathPayload);
    }

    async function handleDuplicateRecentFile(file: RecentFileRecord): Promise<void> {
        try {
            const newPath = await duplicateFile(file.path);
            reportLibraryMutation({
                kind: "duplicated",
                path: newPath,
                source: file.source,
            });
            const newName = newPath.replace(/\\/g, "/").split("/").pop() ?? "copy";
            toast.success(`Duplicated as "${newName}"`);
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            toast.error(`Failed to duplicate: ${msg}`);
        }
    }

    async function handleDuplicateLocalFileAsSlate(file: RecentFileRecord): Promise<void> {
        try {
            const newPath = await duplicateLocalFileAsSlate(file.path);
            reportLibraryMutation({
                kind: "duplicated",
                path: newPath,
                source: "slates",
            });
            const newName = newPath.replace(/\\/g, "/").split("/").pop() ?? "copy";
            toast.success(`Duplicated as slate "${newName}"`);
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            toast.error(`Failed to duplicate as slate: ${msg}`);
        }
    }

    function restoreRemovedRecord<T extends { path: string }>(
        records: T[],
        record: T | undefined,
        index: number,
    ): T[] {
        if (!record || records.some((item) => item.path === record.path)) return records;

        const restored = [...records];
        restored.splice(Math.min(Math.max(index, 0), restored.length), 0, record);
        return restored;
    }

    async function handleUnlink(file: RecentFileRecord): Promise<void> {
        // If this is the file currently open in the editor, confirm before
        // discarding any unsaved changes. Must happen before the unlink
        // actually runs, not after — otherwise the file is already
        // untracked by the time the user sees the prompt.
        if (file.path === editorState.currentFilePath) {
            if (!(await confirmBeforeLeavingDocument())) return;
        }

        // Optimistically remove from both lists before awaiting the backend so
        // the card disappears immediately without a visible delay.
        const recentIndex = recentFiles.findIndex((item) => item.path === file.path);
        const removedRecentFile = recentFiles[recentIndex];
        const searchIndex = searchResults.findIndex((item) => item.path === file.path);
        const removedSearchResult = searchResults[searchIndex];
        recentFiles = recentFiles.filter((f) => f.path !== file.path);
        searchResults = searchResults.filter((f) => f.path !== file.path);

        try {
            await performFileUnlink(file);
        } catch (err) {
            // Restore only the removed record so concurrent refreshes or
            // mutations are not overwritten by an old whole-list snapshot.
            recentFiles = restoreRemovedRecord(recentFiles, removedRecentFile, recentIndex);
            searchResults = restoreRemovedRecord(searchResults, removedSearchResult, searchIndex);
            const msg = err instanceof Error ? err.message : String(err);
            toast.error(`Failed to unlink: ${msg}`);
        }
    }

    function handleRefresh(): void {
        clearReorderSuppression();
        navigator.reset();
        refreshCoordinator.requestActive({ priority: "immediate", showLoading: true });
    }

    function requestSearchFocus(): void {
        focusSearchRequest += 1;
    }

    function activateLibrarySearch(): void {
        filterMode = DEFAULT_FILTER_MODE;
        query = "";
        searchOptions = { ...DEFAULT_SEARCH_OPTIONS };
        navigator.reset();

        if (!sidebar.open) {
            sidebar.setOpen(true);
        }

        requestSearchFocus();
    }

    function ensureSourceIsVisible(source: RecentFileSource): void {
        if (filterMode !== "unified" && filterMode !== source) {
            filterMode = "unified";
        }
    }

    function requestImmediateActiveRefresh(): void {
        clearReorderSuppression();
        refreshCoordinator.requestActive({ priority: "immediate" });
    }

    function handleLibraryMutation(mutation: LibraryMutation): void {
        switch (mutation.kind) {
            case "created":
                requestImmediateActiveRefresh();
                return;

            case "opened":
                if (mutation.origin === "external") {
                    pendingRevealPath = mutation.path;
                    query = "";
                    ensureSourceIsVisible(mutation.source);
                    requestImmediateActiveRefresh();
                    return;
                }

                refreshCoordinator.requestActive({ priority: "background" });
                return;

            case "saved":
                // A completed save is a material change. Release the freeze
                // used to prevent an open from reordering the list, then show
                // the saved file at its updated position immediately.
                requestImmediateActiveRefresh();
                return;

            case "duplicated":
                pendingRevealPath = mutation.path;
                query = "";
                ensureSourceIsVisible(mutation.source);
                requestImmediateActiveRefresh();
                return;

            case "removed":
                recentFiles = recentFiles.filter((file) => file.path !== mutation.path);
                searchResults = searchResults.filter((file) => file.path !== mutation.path);
                if (pendingRevealPath === mutation.path) {
                    pendingRevealPath = undefined;
                }
                requestImmediateActiveRefresh();
                return;

            case "renamed":
                if (pendingRevealPath === mutation.from) {
                    pendingRevealPath = mutation.to;
                }
                requestImmediateActiveRefresh();
                return;

            case "sync":
                refreshCoordinator.requestActive({ priority: "background" });
                return;
        }
    }

    $effect(() => {
        librarySidebarState.requestActivateSearch = activateLibrarySearch;
        librarySidebarState.handleLibraryMutation = handleLibraryMutation;

        return () => {
            if (librarySidebarState.requestActivateSearch === activateLibrarySearch) {
                librarySidebarState.requestActivateSearch = undefined;
            }
            if (librarySidebarState.handleLibraryMutation === handleLibraryMutation) {
                librarySidebarState.handleLibraryMutation = undefined;
            }
        };
    });

    // ---------------------------------------------------------------------------
    // Effects
    // ---------------------------------------------------------------------------

    $effect(() => {
        refreshCoordinator.requestRecent({
            priority: "immediate",
            showLoading: true,
        });
    });

    $effect(() => {
        const path = pendingRevealPath;
        if (!path || isSearchMode) {
            return;
        }

        if (!recentFiles.some((file) => file.path === path)) {
            return;
        }

        if (!activeResults.some((file) => file.path === path)) {
            pendingRevealPath = undefined;
            return;
        }

        navigator.revealHighlight(path);
        pendingRevealPath = undefined;
    });

    // Refresh when the sidebar is reopened so the list reflects file changes
    // that occurred while it was collapsed (e.g. deferred save events).
    $effect(() => {
        const isOpen = sidebar.open;
        const wasOpen = lastObservedSidebarOpen;
        lastObservedSidebarOpen = isOpen;

        if (!isOpen && wasOpen && suppressReorder) {
            // Sidebar is closing while list reorder is suppressed. Pre-fetch
            // now (while the sidebar is animating away, invisible to the user)
            // so the correctly-sorted data is already in place when it opens
            // again. No visible transition on reopen.
            suppressReorder = false;
            lastSidebarOpenedPath = undefined;
            refreshCoordinator.releaseDeferred();
            refreshCoordinator.requestRecent({ priority: "immediate" });
        }
    });

    // Debounced search: clears results immediately when query is empty,
    // otherwise waits 120 ms after the last keystroke before firing.
    // Cancels any in-flight backend search immediately on every keystroke
    // so superseded work stops as early as possible.
    // Also re-runs when search options (case, word, regex) are toggled.
    $effect(() => {
        normalizedQuery;
        filterMode;
        sidebar.open;
        searchOptionsKey;

        if (normalizedQuery.length === 0) {
            searchRequestVersion += 1;
            searchResults = [];
            isSearchLoading = false;
            loadError = "";
            navigator.resetToFile(editorState.currentFilePath);
            // Kill any in-flight backend search immediately.
            void cancelSidebarSearch();
            return;
        }

        if (!sidebar.open) {
            return;
        }

        // Cancel the previous backend search right away — don't wait for
        // the replacement debounced request to reach Rust.
        void cancelSidebarSearch();

        const timeoutId = window.setTimeout(() => {
            void refreshSearchResults();
        }, 120);

        return () => {
            window.clearTimeout(timeoutId);
        };
    });

    $effect(() => {
        const currentFilePath = editorState.currentFilePath;
        const isUntitledDocument = editorState.isUntitledDocument;
        const pending = pendingOpenFile;

        const editorLocationChanged = currentFilePath !== lastObservedEditorPath
            || isUntitledDocument !== lastObservedUntitledState;

        lastObservedEditorPath = currentFilePath;
        lastObservedUntitledState = isUntitledDocument;

        if (!editorLocationChanged || pending) {
            return;
        }

        // File was opened from the sidebar — keep the list frozen until an
        // explicit user action (sort change, manual refresh, sidebar reopen).
        if (suppressReorder && currentFilePath && currentFilePath === lastSidebarOpenedPath) {
            return;
        }

        // Edge case: suppression is active but the loaded file doesn't match
        // the sidebar-clicked path (e.g. a concurrent external open won the
        // race). Clear it so the list isn't stuck frozen indefinitely.
        if (suppressReorder) {
            clearReorderSuppression();
        }
    });

    $effect(() => {
        const nextFilterMode = filterMode;
        const nextSortMode = sortMode;

        const filterChanged = nextFilterMode !== lastObservedFilterMode;
        const sortChanged = nextSortMode !== lastObservedSortMode;

        lastObservedFilterMode = nextFilterMode;
        lastObservedSortMode = nextSortMode;

        if (!filterChanged && !sortChanged) {
            return;
        }

        navigator.scrollToTop();
        navigator.resetToFile(editorState.currentFilePath);

        if (isSearchMode) {
            clearReorderSuppression();
            refreshCoordinator.requestActive({ priority: "immediate" });
            return;
        }

        // Pure filter (tab) change while the list is frozen from a sidebar
        // open: skip the fetch entirely. The visibleRecentFiles derived
        // already re-filters the existing data under the new tab — the only
        // difference in the DB is the opened_at bump on the file the user
        // just clicked, and surfacing that reorder mid-browse is the jitter
        // we want to avoid. Suppression clears on sort change, manual
        // refresh, sidebar reopen, or opening a file from outside.
        if (suppressReorder && filterChanged && !sortChanged) {
            return;
        }

        clearReorderSuppression();
        refreshCoordinator.requestRecent({ priority: "immediate" });
    });

    $effect(() => {
        return registerHotkey("Mod+P", (event) => {
            event.preventDefault();
            librarySidebarState.requestActivateSearch?.();
        }, { ignoreInputs: false });
    });

    $effect(() => {
        let disposed = false;
        let unlistenRecentFiles: undefined | (() => void);

        const setup = import("@tauri-apps/api/event").then(async ({ listen }) => {
            unlistenRecentFiles = await listen<"saved" | null>(RECENT_FILES_UPDATED_EVENT, (event) => {
                if (disposed) {
                    return;
                }

                reportLibraryMutation(event.payload === "saved" ? { kind: "saved" } : { kind: "sync" });
            });
        });

        return () => {
            disposed = true;
            refreshCoordinator.destroy();
            // Kill any in-flight search when the sidebar component unmounts
            // so the backend doesn't keep working on a stale request.
            void cancelSidebarSearch();
            setup.finally(() => {
                unlistenRecentFiles?.();
            });
        };
    });
</script>

<div
    class="flex h-full w-full flex-col bg-sidebar text-sidebar-foreground"
    onfocusin={() => { isSidebarFocused = true; }}
    onfocusout={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node)) isSidebarFocused = false; }}
>
    <SidebarHeader
        bind:query
        bind:filterMode
        bind:sortMode
        bind:searchOptions
        {isLoading}
        {isSearchLoading}
        focusRequest={focusSearchRequest}
        onRefresh={handleRefresh}
        navigationHotkeys={navigator.inputHotkeys}
    />

    <SidebarFileList
        bind:scrollContainer={resultsScrollContainer}
        sections={recentFileSections}
        showExternalBadge={filterMode === "unified"}
        {isSearchMode}
        {isLoading}
        {isSearchLoading}
        {activeResults}
        {loadError}
        highlightedPath={navigator.highlightedPath}
        pendingOpenFilePath={pendingOpenFile?.path}
        currentFilePath={editorState.currentFilePath}
        onOpen={openRecentFile}
        onHighlight={navigator.handleHighlight}
        listHotkeys={navigator.listHotkeys}
        onDuplicate={handleDuplicateRecentFile}
        onDuplicateAsSlate={handleDuplicateLocalFileAsSlate}
        onUnlink={handleUnlink}
    />
</div>
