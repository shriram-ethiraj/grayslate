<script lang="ts">
    import { registerHotkey } from "$lib/hotkeys";
    import * as Sidebar from "$lib/components/ui/sidebar/index.js";
    import { editorState } from "$lib/state/editor.svelte";
    import {
        librarySidebarState,
        setPendingSidebarOpenFile,
    } from "$lib/state/librarySidebar.svelte";
    import {
        getRecentFiles,
        OPEN_FILE_PATH_EVENT,
        RECENT_FILES_UPDATED_EVENT,
        type OpenFilePathPayload,
        type RecentFileRecord,
        type RecentFileSource,
        type SidebarSearchResult,
        searchSidebarFiles,
        duplicateFile,
        duplicateLocalFileAsSlate,
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

    // ---------------------------------------------------------------------------
    // Component state
    // ---------------------------------------------------------------------------

    let query = $state("");
    let filterMode = $state<FilterMode>(DEFAULT_FILTER_MODE);
    let sortMode = $state<SortMode>(DEFAULT_SORT_MODE);

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
    //   • The derived `visibleRecentFiles` skips re-sorting.
    //   • RECENT_FILES_UPDATED_EVENT refreshes are silently deferred.
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
    //   • Backend events (file save, rename, duplicate, delete).
    //   • Filter (tab) changes while suppression is active.
    //   • Rename of the active file (tracking path is updated instead).
    // ---------------------------------------------------------------------------
    let suppressReorder = $state(false);
    // The path of the last file opened via the sidebar, used to decide whether
    // to keep suppressReorder active when the editor navigation event fires.
    let lastSidebarOpenedPath = $state<string | undefined>(undefined);

    // Incrementing version counters for in-flight async requests; a stale
    // response whose version doesn't match the current one is discarded.
    let recentFilesRequestVersion = 0;
    let searchRequestVersion = 0;

    // DOM ref for the scrollable results container (propagated from SidebarFileList).
    let resultsScrollContainer = $state<HTMLDivElement | null>(null);
    // Incrementing counter: bump to request focus of the search input in SidebarHeader.
    let focusSearchRequest = $state(0);

    // Previous values used to detect *changes* in effects without triggering
    // on the initial run. Initialized to the current values so the first pass
    // is always a no-op.
    let lastObservedEditorPath: string | undefined = editorState.currentFilePath;
    let lastObservedUntitledState = editorState.isUntitledDocument;
    let lastObservedFilterMode: FilterMode = DEFAULT_FILTER_MODE;
    let lastObservedSortMode: SortMode = DEFAULT_SORT_MODE;
    let lastObservedSidebarOpen = true;

    // ---------------------------------------------------------------------------
    // Derived state
    // ---------------------------------------------------------------------------

    const sidebar = Sidebar.useSidebar();

    const normalizedQuery = $derived(query.trim().toLowerCase());
    const pendingOpenFile = $derived(librarySidebarState.pendingOpenFile);

    /** Whitespace-split search terms for frontend highlight rendering. */
    const searchTerms = $derived(
        normalizedQuery.length > 0
            ? normalizedQuery.split(/\s+/).filter((t: string) => t.length > 0)
            : [],
    );

    const visibleRecentFiles = $derived.by(() => {
        const filteredRecentFiles = recentFiles.filter((recentFile) =>
            filterMode === "unified" || recentFile.source === filterMode
        );

        // Skip re-sorting while the list is frozen so the order doesn't jump
        // mid-session when the user has just opened a file.
        if (suppressReorder && normalizedQuery.length === 0) {
            return filteredRecentFiles;
        }

        filteredRecentFiles.sort((left, right) => compareRecentFiles(left, right, sortMode));
        return filteredRecentFiles;
    });

    const sortedSearchResults = $derived.by(() => {
        const sorted = [...searchResults];
        sorted.sort((left, right) => compareSearchResults(left, right, sortMode));
        return sorted;
    });

    const activeResults = $derived<LibraryFileRecord[]>(
        normalizedQuery.length === 0 ? visibleRecentFiles : sortedSearchResults,
    );

    const recentFileSections = $derived.by((): RecentFileSection[] => {
        if (
            normalizedQuery.length > 0 ||
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
    }

    // ---------------------------------------------------------------------------
    // Data fetching
    // ---------------------------------------------------------------------------

    async function fetchRecentFiles(options?: {
        showLoading?: boolean;
        clearSuppression?: boolean;
    }): Promise<void> {
        const showLoading = options?.showLoading ?? true;
        const clearSuppression = options?.clearSuppression ?? false;
        const currentVersion = ++recentFilesRequestVersion;
        if (showLoading) {
            isLoading = true;
        }

        if (showLoading && normalizedQuery.length === 0) {
            loadError = "";
        }

        try {
            const result = await getRecentFiles(RECENT_FILES_LIMIT);
            if (currentVersion !== recentFilesRequestVersion) {
                return;
            }

            // Clear suppression and update data in the same synchronous block so
            // Svelte batches them into one render. This prevents the two-step
            // jitter: "list re-sorts on old data" → "list updates with new data".
            if (clearSuppression) {
                suppressReorder = false;
                lastSidebarOpenedPath = undefined;
            }
            recentFiles = result;
        } catch (error: unknown) {
            if (currentVersion !== recentFilesRequestVersion) {
                return;
            }

            if (showLoading && normalizedQuery.length === 0) {
                loadError = typeof error === "string"
                    ? error
                    : "Failed to load recent files.";
            }
        } finally {
            if (showLoading && currentVersion === recentFilesRequestVersion) {
                isLoading = false;
            }
        }
    }

    async function refreshRecentFiles(): Promise<void> {
        await fetchRecentFiles({ showLoading: true });
    }

    /** Silently refresh without showing a loading skeleton. Used for
     *  background syncs (backend events, tab switches, sidebar reveal). */
    async function quietRefreshRecentFiles(options?: { clearSuppression?: boolean }): Promise<void> {
        await fetchRecentFiles({ showLoading: false, clearSuppression: options?.clearSuppression });
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
            );
            if (currentVersion !== searchRequestVersion) {
                return;
            }

            searchResults = result;
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

    // ---------------------------------------------------------------------------
    // UI actions
    // ---------------------------------------------------------------------------

    async function openRecentFile(path: string, source: RecentFileSource, lineNumber?: number): Promise<void> {
        // Freeze the list order so opening a file doesn't immediately re-sort
        // the sidebar, which would be jarring for sequential file navigation.
        suppressReorder = true;
        lastSidebarOpenedPath = path;

        const requestId = Date.now();
        setPendingSidebarOpenFile({
            path,
            source,
            requestId,
            revealInRecentList: false,
            lineNumber,
        });

        const { emit } = await import("@tauri-apps/api/event");
        await emit(OPEN_FILE_PATH_EVENT, { path, lineNumber } satisfies OpenFilePathPayload);
    }

    async function handleDuplicateRecentFile(file: RecentFileRecord): Promise<void> {
        try {
            const newPath = await duplicateFile(file.path);
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
            const newName = newPath.replace(/\\/g, "/").split("/").pop() ?? "copy";
            toast.success(`Duplicated as slate "${newName}"`);
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            toast.error(`Failed to duplicate as slate: ${msg}`);
        }
    }

    function handleRefresh(): void {
        clearReorderSuppression();
        if (normalizedQuery.length > 0) {
            void refreshSearchResults();
        } else {
            void refreshRecentFiles();
        }
    }

    function requestSearchFocus(): void {
        focusSearchRequest += 1;
    }

    function scrollResultsToTop(): void {
        resultsScrollContainer?.scrollTo({ top: 0, behavior: "auto" });
    }

    function activateLibrarySearch(): void {
        filterMode = DEFAULT_FILTER_MODE;
        sortMode = DEFAULT_SORT_MODE;
        query = "";

        if (!sidebar.open) {
            sidebar.setOpen(true);
        }

        requestSearchFocus();
    }

    $effect(() => {
        librarySidebarState.requestActivateSearch = activateLibrarySearch;
        // Registered for the rename dialog: refreshes cached file data (new
        // filename, new path) while keeping suppressReorder active so the
        // visible list order doesn't shift.
        librarySidebarState.requestQuietDataRefresh = () => {
            void fetchRecentFiles({ showLoading: false, clearSuppression: false });
        };

        return () => {
            if (librarySidebarState.requestActivateSearch === activateLibrarySearch) {
                librarySidebarState.requestActivateSearch = undefined;
            }
            librarySidebarState.requestQuietDataRefresh = undefined;
        };
    });

    // ---------------------------------------------------------------------------
    // Effects
    // ---------------------------------------------------------------------------

    $effect(() => {
        void refreshRecentFiles();
    });

    // Refresh when the sidebar is reopened so the list reflects any changes
    // that occurred while it was collapsed (e.g. suppressed open events).
    $effect(() => {
        const isOpen = sidebar.open;
        const wasOpen = lastObservedSidebarOpen;
        lastObservedSidebarOpen = isOpen;

        if (!isOpen && wasOpen && suppressReorder) {
            // Sidebar is closing while list reorder is suppressed. Pre-fetch
            // now (while the sidebar is animating away, invisible to the user)
            // so the correctly-sorted data is already in place when it opens
            // again. No visible transition on reopen.
            void quietRefreshRecentFiles({ clearSuppression: true });
        }
    });

    // Debounced search: clears results immediately when query is empty,
    // otherwise waits 120 ms after the last keystroke before firing.
    $effect(() => {
        normalizedQuery;
        filterMode;
        sidebar.open;

        if (normalizedQuery.length === 0) {
            searchRequestVersion += 1;
            searchResults = [];
            isSearchLoading = false;
            loadError = "";
            return;
        }

        if (!sidebar.open) {
            return;
        }

        const timeoutId = window.setTimeout(() => {
            void refreshSearchResults();
        }, 120);

        return () => {
            window.clearTimeout(timeoutId);
        };
    });

    $effect(() => {
        const pending = pendingOpenFile;
        if (!pending?.revealInRecentList) {
            return;
        }

        clearReorderSuppression();
        // Switch to "unified" so the file is visible regardless of its actual
        // source (slates vs local). We don't know the backend-classified
        // source at this point; "unified" guarantees visibility.
        if (filterMode !== "unified") {
            filterMode = "unified";
        }
        if (query.length > 0) {
            query = "";
        }
    });

    $effect(() => {
        const currentFilePath = editorState.currentFilePath;
        const isUntitledDocument = editorState.isUntitledDocument;
        const pending = pendingOpenFile;
        const renamedPath = librarySidebarState.lastRenamedPath;

        const editorLocationChanged = currentFilePath !== lastObservedEditorPath
            || isUntitledDocument !== lastObservedUntitledState;

        lastObservedEditorPath = currentFilePath;
        lastObservedUntitledState = isUntitledDocument;

        if (!editorLocationChanged || pending) {
            return;
        }

        // The active file was renamed — update suppression tracking to the
        // new path so we don't misinterpret the rename as an external open.
        if (renamedPath && suppressReorder && lastSidebarOpenedPath === renamedPath.from) {
            lastSidebarOpenedPath = renamedPath.to;
            librarySidebarState.lastRenamedPath = undefined;
            return;
        }

        // Consume the signal even if suppression wasn't active.
        if (renamedPath) {
            librarySidebarState.lastRenamedPath = undefined;
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

        scrollResultsToTop();

        if (normalizedQuery.length > 0) {
            clearReorderSuppression();
            void refreshSearchResults();
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

        void quietRefreshRecentFiles({ clearSuppression: true });
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
            unlistenRecentFiles = await listen(RECENT_FILES_UPDATED_EVENT, () => {
                if (disposed) {
                    return;
                }

                // When the user opened a file from the sidebar, skip the
                // refresh so the list doesn't reorder under their cursor.
                // The data will catch up on the next user action that clears
                // suppressReorder (tab switch, sort change, manual refresh,
                // or opening a file from outside the sidebar).
                if (suppressReorder) {
                    return;
                }

                void quietRefreshRecentFiles();
            });
        });

        return () => {
            disposed = true;
            setup.finally(() => {
                unlistenRecentFiles?.();
            });
        };
    });
</script>

<div class="flex h-full w-full flex-col bg-sidebar text-sidebar-foreground">
    <SidebarHeader
        bind:query
        bind:filterMode
        bind:sortMode
        {isLoading}
        {isSearchLoading}
        focusRequest={focusSearchRequest}
        onRefresh={handleRefresh}
    />

    <SidebarFileList
        bind:scrollContainer={resultsScrollContainer}
        sections={recentFileSections}
        {normalizedQuery}
        {searchTerms}
        {isLoading}
        {isSearchLoading}
        {activeResults}
        {loadError}
        pendingOpenFilePath={pendingOpenFile?.path}
        currentFilePath={editorState.currentFilePath}
        onOpen={openRecentFile}
        onDuplicate={handleDuplicateRecentFile}
        onDuplicateAsSlate={handleDuplicateLocalFileAsSlate}
    />
</div>
