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

    // `recentFiles` is the list currently shown. `stagedRecentFiles` holds a
    // fresh fetch that arrived while the list was frozen (suppressReorder=true);
    // it is promoted to `recentFiles` on the next explicit user action.
    let recentFiles = $state<RecentFileRecord[]>([]);
    let stagedRecentFiles = $state<RecentFileRecord[] | undefined>(undefined);

    let searchResults = $state<SidebarSearchResult[]>([]);
    let isLoading = $state(false);
    let isSearchLoading = $state(false);
    let loadError = $state("");

    // When true, incoming RECENT_FILES_UPDATED_EVENT updates are staged instead
    // of applied immediately, preserving the current visual order. Set when the
    // user opens a file from the sidebar; cleared on any explicit user action
    // (tab change, sort change, refresh, or opening a file from outside).
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
        applyToVisibleList?: boolean;
        showLoading?: boolean;
    }): Promise<void> {
        const applyToVisibleList = options?.applyToVisibleList ?? true;
        const showLoading = options?.showLoading ?? true;
        const currentVersion = ++recentFilesRequestVersion;
        if (showLoading) {
            isLoading = true;
        }

        if (showLoading && normalizedQuery.length === 0) {
            loadError = "";
        }

        try {
            const result = await getRecentFiles(120);
            if (currentVersion !== recentFilesRequestVersion) {
                return;
            }

            if (applyToVisibleList) {
                recentFiles = result;
                stagedRecentFiles = undefined;
            } else {
                stagedRecentFiles = result;
            }
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
        await fetchRecentFiles({
            applyToVisibleList: true,
            showLoading: true,
        });
    }

    async function stageRecentFilesUpdate(): Promise<void> {
        await fetchRecentFiles({
            applyToVisibleList: false,
            showLoading: false,
        });
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
            // Clear reorder suppression and refresh directly so the new file
            // is always visible immediately, even if suppressReorder is active
            // from a prior sidebar-initiated file open.
            clearReorderSuppression();
            void refreshRecentFiles();
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
            clearReorderSuppression();
            // Switch to the Slates tab so the new file is immediately visible.
            filterMode = "slates";
            void refreshRecentFiles();
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

        return () => {
            if (librarySidebarState.requestActivateSearch === activateLibrarySearch) {
                librarySidebarState.requestActivateSearch = undefined;
            }
        };
    });

    // ---------------------------------------------------------------------------
    // Effects
    // ---------------------------------------------------------------------------

    $effect(() => {
        void refreshRecentFiles();
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
        filterMode = pending.source;
        if (query.length > 0) {
            query = "";
        }
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

        // File was opened from the sidebar — keep the list frozen until a user action.
        if (suppressReorder && currentFilePath && currentFilePath === lastSidebarOpenedPath) {
            return;
        }

        if (suppressReorder || currentFilePath || isUntitledDocument) {
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

        clearReorderSuppression();
        scrollResultsToTop();

        if (normalizedQuery.length > 0) {
            void refreshSearchResults();
            return;
        }

        // Apply any staged data that arrived while the list was frozen.
        if (filterChanged && stagedRecentFiles) {
            recentFiles = stagedRecentFiles;
            stagedRecentFiles = undefined;
        }
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

                // Only stage the refresh when a file-open navigation is actively
                // in flight (pendingOpenFile). The suppressReorder flag only
                // freezes the sort order (handled in visibleRecentFiles), so
                // mutations like rename/delete/duplicate still refresh immediately.
                if (pendingOpenFile) {
                    void stageRecentFilesUpdate();
                    return;
                }

                void refreshRecentFiles();
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
