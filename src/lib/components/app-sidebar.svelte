<script lang="ts">
    import { tick } from "svelte";
    import { writeText } from "@tauri-apps/plugin-clipboard-manager";
    import { revealItemInDir } from "@tauri-apps/plugin-opener";
    import { toast } from "$lib/components/ui/sonner";
    import { registerHotkey } from "$lib/hotkeys";
    import type { LanguageIcon } from "$lib/editor/config/supportedLanguages";
    import { languages } from "$lib/editor/config/supportedLanguages";
    import { languageDetector } from "$lib/editor/core/languageDetector";
    import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
    import * as Sidebar from "$lib/components/ui/sidebar/index.js";
    import * as Item from "$lib/components/ui/item/index.js";
    import * as Select from "$lib/components/ui/select/index.js";
    import * as Tabs from "$lib/components/ui/tabs/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import Input from "$lib/components/ui/input/input.svelte";
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
        searchSidebarFiles,
        type SidebarSearchResult,
    } from "$lib/files/recentFiles";
    import {
        buildRecencySections,
        compareRecentFiles,
        compareSearchResults,
        DEFAULT_FILTER_MODE,
        DEFAULT_SORT_MODE,
        formatSize,
        formatTimestamp,
        getDirectoryLabel,
        getRecencyTimestamp,
        getLineExcerpt,
        isSearchResult,
        RECENT_FILES_LIMIT,
        recencySectionOrder,
        splitTextByTerms,
        type FilterMode,
        type LibraryFileRecord,
        type RecencyBucket,
        type RecentFileSection,
        type SortMode,
    } from "$lib/files/sidebarUtils";
    import Search from "~icons/lucide/search";
    import RefreshCcw from "~icons/lucide/refresh-ccw";
    import Files from "~icons/lucide/files";
    import Clock3 from "~icons/lucide/clock-3";
    import History from "~icons/lucide/history";
    import ArrowDownAZ from "~icons/lucide/arrow-down-a-z";
    import ArrowUpZA from "~icons/lucide/arrow-up-z-a";
    import ArrowDownWideNarrow from "~icons/lucide/arrow-down-wide-narrow";
    import ArrowUpNarrowWide from "~icons/lucide/arrow-up-narrow-wide";
    import RiCodeBoxLine from '~icons/ri/code-box-line';
    import FolderOpen from "~icons/lucide/folder-open";
    import Copy from "~icons/lucide/copy";
    import CopyPlus from "~icons/lucide/copy-plus";
    import Pencil from "~icons/lucide/pencil";
    import Trash2 from "~icons/lucide/trash-2";
    import LucideHardDrive from '~icons/lucide/hard-drive';
    import FileWarning from "~icons/lucide/file-warning";
    import { getPlatformOsType } from "$lib/state/platform.svelte";
    import { openDeleteFileDialog, openRenameFileDialog } from "$lib/state/appDialogs.svelte";
    import { duplicateFile, duplicateLocalFileAsSlate } from "$lib/files/recentFiles";

    // ---------------------------------------------------------------------------
    // Module-level constants (rendering only — types/sort/format live in sidebarUtils.ts)
    // ---------------------------------------------------------------------------

    const languageMetaByValue = new Map(languages.map((language) => [language.value, language] as const));

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

    // DOM refs
    let searchInput = $state<HTMLInputElement | null>(null);
    let resultsScrollContainer = $state<HTMLDivElement | null>(null);
    // Incrementing counter: bump to request focus of the search input.
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

    const recentFileSections = $derived.by(() => {
        if (normalizedQuery.length > 0 || (sortMode !== "recently-opened" && sortMode !== "least-recently-opened")) {
            return [{
                key: "all",
                label: "",
                items: activeResults,
            }] satisfies RecentFileSection[];
        }

        return buildRecencySections(activeResults, sortMode);
    });

    // ---------------------------------------------------------------------------
    // Static option lists (defined here because they reference icon components)
    // ---------------------------------------------------------------------------

    const filterOptions: Array<{
        value: FilterMode;
        label: string;
        title: string;
        icon: typeof Files;
    }> = [
        {
            value: "unified",
            label: "All",
            title: "Show all recently opened files",
            icon: Files,
        },
        {
            value: "slates",
            label: "Slates",
            title: "Show Grayslate documents only",
            icon: RiCodeBoxLine,
        },
        {
            value: "local",
            label: "Local",
            title: "Show previously opened local files only",
            icon: LucideHardDrive,
        },
    ];

    const sortOptions: Array<{
        value: SortMode;
        label: string;
        icon: typeof Search;
    }> = [
        { value: "recently-opened", label: "Recently opened", icon: Clock3 },
        { value: "least-recently-opened", label: "Least recently opened", icon: History },
        { value: "name-asc", label: "Name (A to Z)", icon: ArrowDownAZ },
        { value: "name-desc", label: "Name (Z to A)", icon: ArrowUpZA },
        { value: "size-desc", label: "Largest first", icon: ArrowDownWideNarrow },
        { value: "size-asc", label: "Smallest first", icon: ArrowUpNarrowWide },
    ];

    const activeSortOption = $derived(
        sortOptions.find((option) => option.value === sortMode) ?? sortOptions[0],
    );

    // ---------------------------------------------------------------------------
    // Language / display helpers (depend on languageMetaByValue, stay here)
    // ---------------------------------------------------------------------------

    function getRecentFileTypeToken(recentFile: LibraryFileRecord): string {
        const normalizedExtension = recentFile.extension?.replace(/^\./, "").trim().toUpperCase();
        if (normalizedExtension) {
            return normalizedExtension;
        }

        const detectedLanguage = getRecentFileLanguage(recentFile);
        const languageMeta = languageMetaByValue.get(detectedLanguage);
        if (languageMeta?.token) {
            return languageMeta.token;
        }

        return languageMeta?.label.toUpperCase() ?? "FILE";
    }

    function getRecentFileLanguage(recentFile: LibraryFileRecord): string {
        return languageDetector.detect("", recentFile.file_name)
            ?? languageDetector.detect("", recentFile.path)
            ?? "text";
    }

    function getRecentFileIcon(recentFile: LibraryFileRecord): LanguageIcon | null {
        return languageMetaByValue.get(getRecentFileLanguage(recentFile))?.icon
            ?? languageMetaByValue.get("text")?.icon
            ?? null;
    }

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
            const result = await getRecentFiles(RECENT_FILES_LIMIT);
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

    function getRevealInFileManagerLabel(): string {
        switch (getPlatformOsType()) {
            case "macos":
                return "Show in Finder";
            case "linux":
                return "Show in File Manager";
            default:
                return "Show in Explorer";
        }
    }

    async function handleCopyRecentFilePath(path: string): Promise<void> {
        try {
            await writeText(path);
        } catch {
            toast.error("Failed to copy path");
        }
    }

    async function handleRevealRecentFile(path: string): Promise<void> {
        try {
            await revealItemInDir(path);
        } catch {
            toast.error("Failed to open containing folder");
        }
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
            void refreshRecentFiles();
            const newName = newPath.replace(/\\/g, "/").split("/").pop() ?? "copy";
            toast.success(`Duplicated as slate "${newName}"`);
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            toast.error(`Failed to duplicate as slate: ${msg}`);
        }
    }

    function requestSearchFocus(): void {
        focusSearchRequest += 1;
    }

    function scrollResultsToTop(): void {
        if (!resultsScrollContainer) {
            return;
        }

        resultsScrollContainer.scrollTo({
            top: 0,
            behavior: "auto",
        });
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
        focusSearchRequest;

        if (!sidebar.open || !searchInput) {
            return;
        }

        let cancelled = false;

        void tick().then(() => {
            requestAnimationFrame(() => {
                if (cancelled || !sidebar.open || !searchInput) {
                    return;
                }

                searchInput.focus();
                searchInput.setSelectionRange(query.length, query.length);
            });
        });

        return () => {
            cancelled = true;
        };
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

                if (pendingOpenFile || suppressReorder) {
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
    <Sidebar.Group class="shrink-0 gap-2 border-b border-sidebar-border/70 px-2 py-2">
        <div class="flex items-center justify-between gap-2 px-1">
            <div class="min-w-0 truncate text-sm font-medium">Library</div>
            <Button
                variant="ghost"
                size="icon-sm"
                class="text-sidebar-foreground/70 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
                aria-label="Refresh recent files"
                title="Refresh recent files"
                onclick={() => {
                    clearReorderSuppression();
                    if (normalizedQuery.length > 0) {
                        void refreshSearchResults();
                    } else {
                        void refreshRecentFiles();
                    }
                }}
            >
                <RefreshCcw class={isLoading || isSearchLoading ? "size-4 animate-spin" : "size-4"} />
            </Button>
        </div>

        <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2 px-1">
            <div class="relative min-w-0">
                <Search class="pointer-events-none absolute left-4 top-1/2 z-10 size-4 -translate-y-1/2 text-sidebar-foreground" />
                <Input
                    bind:ref={searchInput}
                    bind:value={query}
                    clearable
                    placeholder="Search library..."
                    class="border-sidebar-border bg-sidebar ps-9 text-sm shadow-none placeholder:text-sidebar-foreground/45 focus-visible:border-sidebar-ring focus-visible:ring-sidebar-ring"
                />
            </div>

            <Select.Root type="single" items={sortOptions} bind:value={sortMode}>
                <Select.Trigger
                    aria-label={`Sort library: ${activeSortOption.label}`}
                    title={`Sort library: ${activeSortOption.label}`}
                    class="h-9 w-9 justify-center gap-0 border-sidebar-border bg-sidebar px-0 text-sidebar-foreground shadow-none focus-visible:border-sidebar-ring focus-visible:ring-sidebar-ring [&>svg:last-child]:hidden"
                >
                    {@const ActiveSortIcon = activeSortOption.icon}
                    <span class="flex items-center justify-center">
                        <ActiveSortIcon class="size-4 text-sidebar-foreground" />
                    </span>
                </Select.Trigger>
                <Select.Content class="border-sidebar-border bg-sidebar text-sidebar-foreground">
                    {#each sortOptions as option (option.value)}
                        {@const OptionIcon = option.icon}
                        <Select.Item value={option.value} label={option.label}>
                            <span class="flex items-center gap-2">
                                <OptionIcon class="size-4" />
                                <span>{option.label}</span>
                            </span>
                        </Select.Item>
                    {/each}
                </Select.Content>
            </Select.Root>
        </div>

        <Tabs.Root bind:value={filterMode}>
            <Tabs.List class="grid h-10 w-full grid-cols-3 bg-sidebar-accent/45 px-1">
                {#each filterOptions as option (option.value)}
                    {@const Icon = option.icon}
                    <Tabs.Trigger
                        value={option.value}
                        class="min-w-0 gap-1 overflow-hidden px-2 text-xs text-sidebar-foreground/75 data-[state=active]:bg-sidebar data-[state=active]:text-sidebar-foreground"
                        title={option.title}
                    >
                        <Icon class="size-3.5" />
                        <span class="min-w-0 truncate">{option.label}</span>
                    </Tabs.Trigger>
                {/each}
            </Tabs.List>
        </Tabs.Root>
    </Sidebar.Group>

    <div bind:this={resultsScrollContainer} class="flex-1 min-h-0 overflow-auto p-2">
        <Sidebar.Group class="gap-2 p-0">
            {#if loadError}
                <div class="rounded-lg border border-destructive/30 bg-destructive/8 px-3 py-2 text-sm text-destructive">
                    {loadError}
                </div>
            {:else if ((normalizedQuery.length === 0 && isLoading && recentFiles.length === 0)
                || (normalizedQuery.length > 0 && isSearchLoading && activeResults.length === 0))}
                <div class="space-y-2 px-1 pt-1">
                    {#each Array.from({ length: 5 }) as _, index (index)}
                        <div class="rounded-lg border border-sidebar-border/60 bg-sidebar-accent/40 px-3 py-3 animate-pulse">
                            <div class="h-3 w-3/5 rounded bg-sidebar-foreground/10"></div>
                            <div class="mt-2 h-2.5 w-4/5 rounded bg-sidebar-foreground/10"></div>
                        </div>
                    {/each}
                </div>
            {:else if activeResults.length === 0}
                <div class="flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-sidebar-border/70 px-4 py-10 text-center text-sm text-sidebar-foreground/65">
                    <Files class="size-5 text-sidebar-foreground/45" />
                    <div>{normalizedQuery.length === 0 ? "No recent files yet." : "No files match this search."}</div>
                </div>
            {:else}
                <Sidebar.GroupContent class="space-y-4 px-1 pb-2">
                    {#each recentFileSections as section (section.key)}
                        <section class="space-y-2">
                            {#if section.label}
                                <div class="flex items-center gap-3 px-2 pt-1">
                                    <span class="truncate whitespace-nowrap text-xs font-semibold uppercase tracking-[0.12em] text-sidebar-foreground/70">
                                        {section.label}
                                    </span>
                                    <div class="h-px flex-1 bg-sidebar-border/70"></div>
                                    <span class="shrink-0 whitespace-nowrap text-xs text-sidebar-foreground/60">
                                        {section.items.length}
                                    </span>
                                </div>
                            {/if}

                            <Item.Group class="gap-2">
                                {#each section.items as recentFile (recentFile.path)}
                                    {@const FileIcon = getRecentFileIcon(recentFile)}
                                    {@const fileSize = formatSize(recentFile.size_bytes)}
                                    {@const isPendingFile = pendingOpenFile?.path === recentFile.path}
                                    {@const isActiveFile = pendingOpenFile
                                        ? isPendingFile
                                        : (!!editorState.currentFilePath && editorState.currentFilePath === recentFile.path)}
                                    {@const searchResult = isSearchResult(recentFile) ? recentFile : null}
                                    <ContextMenu.Root>
                                        <Item.Root
                                            variant="outline"
                                            size="sm"
                                            class="border-0 p-0 shadow-none [transform:translateZ(0)] {isActiveFile ? 'ring-1 ring-inset ring-sidebar-ring bg-sidebar-foreground/[0.03]' : 'ring-1 ring-inset ring-sidebar-border/65 bg-sidebar/35'}"
                                        >
                                            <div class="w-full overflow-hidden rounded-[inherit]">
                                                <ContextMenu.Trigger>
                                                    {#snippet child({ props })}
                                                        <button
                                                            {...props}
                                                            type="button"
                                                            class="group flex w-full min-w-0 items-start gap-3 px-3.5 py-3 text-left outline-none transition-colors {isActiveFile ? 'bg-sidebar-foreground/[0.04] text-sidebar-foreground' : 'hover:bg-sidebar-accent/70 hover:text-sidebar-accent-foreground data-[state=open]:bg-sidebar-accent/70 data-[state=open]:text-sidebar-accent-foreground'}"
                                                            title={recentFile.path}
                                                            onclick={() => {
                                                                void openRecentFile(recentFile.path, recentFile.source);
                                                            }}
                                                        >
                                                            <Item.Media
                                                                variant="icon"
                                                                class="mt-0.5 {isActiveFile ? 'border-sidebar-ring/40 bg-sidebar-foreground/[0.04] text-sidebar-foreground' : 'border-sidebar-border/70 bg-sidebar-accent/45 text-sidebar-foreground/80 group-hover:border-sidebar-background/60 group-hover:bg-sidebar/80 group-hover:text-sidebar-accent-foreground group-data-[state=open]:border-sidebar-background/60 group-data-[state=open]:bg-sidebar/80 group-data-[state=open]:text-sidebar-accent-foreground'}"
                                                            >
                                                                {#if FileIcon}
                                                                    <FileIcon class="size-4.5" />
                                                                {:else}
                                                                    <Files class="size-4.5" />
                                                                {/if}
                                                            </Item.Media>

                                                            <Item.Content class="min-w-0 gap-2.5">
                                                                <div class="flex items-start justify-between gap-3">
                                                                    <div class="min-w-0 flex-1">
                                                                        <Item.Title class="truncate text-sm leading-tight {isActiveFile ? 'text-black dark:text-white' : 'text-sidebar-foreground group-hover:text-sidebar-accent-foreground group-data-[state=open]:text-sidebar-accent-foreground'}">
                                                                            {#if searchTerms.length > 0}
                                                                                {#each splitTextByTerms(recentFile.file_name, searchTerms) as fragment}
                                                                                    {#if fragment.isMatch}<mark class="bg-[var(--selection-match-bg)] text-inherit rounded-sm px-0.5 ring-1 ring-[var(--selection-match-border)]">{fragment.text}</mark>{:else}{fragment.text}{/if}
                                                                                {/each}
                                                                            {:else}
                                                                                {recentFile.file_name}
                                                                            {/if}
                                                                        </Item.Title>

                                                                        <Item.Description class="mt-1 truncate text-xs {isActiveFile ? 'text-black/65 dark:text-white/72' : 'text-sidebar-foreground/62 group-hover:text-sidebar-accent-foreground/74 group-data-[state=open]:text-sidebar-accent-foreground/74'}">
                                                                            {getDirectoryLabel(recentFile.path)}
                                                                        </Item.Description>
                                                                    </div>

                                                                    {#if !recentFile.exists_on_disk}
                                                                        <Item.Actions class="pt-0.5">
                                                                            <span class="inline-flex shrink-0 items-center gap-1 whitespace-nowrap rounded-full border border-amber-500/25 bg-amber-500/10 px-2 py-1 text-xs font-medium uppercase tracking-[0.12em] text-amber-600 dark:text-amber-300">
                                                                                <FileWarning class="size-3.5" />
                                                                                Missing
                                                                            </span>
                                                                        </Item.Actions>
                                                                    {:else if searchResult && searchResult.match_count > 0}
                                                                        <Item.Actions class="pt-0.5">
                                                                            <span class="shrink-0 whitespace-nowrap text-xs tabular-nums {isActiveFile ? 'text-black/60 dark:text-white/65' : 'text-sidebar-foreground/50'}">
                                                                                {searchResult.match_count} {searchResult.match_count === 1 ? "hit" : "hits"}
                                                                            </span>
                                                                        </Item.Actions>
                                                                    {/if}
                                                                </div>

                                                                <div class="flex min-w-0 flex-nowrap items-center gap-2 overflow-hidden text-xs {isActiveFile ? 'text-black/70 dark:text-white/74' : 'text-sidebar-foreground/55 group-hover:text-sidebar-accent-foreground/72 group-data-[state=open]:text-sidebar-accent-foreground/72'}">
                                                                    <span class="truncate whitespace-nowrap font-medium uppercase tracking-[0.12em] {isActiveFile ? 'text-black/80 dark:text-white/88' : 'text-sidebar-foreground/72 group-hover:text-sidebar-accent-foreground/88 group-data-[state=open]:text-sidebar-accent-foreground/88'}">
                                                                        {getRecentFileTypeToken(recentFile)}
                                                                    </span>
                                                                    {#if fileSize}
                                                                        <span aria-hidden="true" class="shrink-0">•</span>
                                                                        <span class="truncate whitespace-nowrap">{fileSize}</span>
                                                                    {/if}
                                                                    <span aria-hidden="true" class="shrink-0">•</span>
                                                                    <span class="truncate whitespace-nowrap">{formatTimestamp(getRecencyTimestamp(recentFile))}</span>
                                                                </div>
                                                            </Item.Content>
                                                        </button>
                                                    {/snippet}
                                                </ContextMenu.Trigger>

                                                {#if searchResult && searchResult.matched_lines.length > 0}
                                                    <div class="border-t border-sidebar-border/40 px-3 py-1.5">
                                                        {#each searchResult.matched_lines as hit (hit.line_number)}
                                                            <button
                                                                type="button"
                                                                class="flex w-full min-w-0 items-baseline gap-2.5 rounded px-1.5 py-1 text-left transition-colors hover:bg-sidebar-accent/50"
                                                                title="Go to line {hit.line_number}"
                                                                onclick={() => {
                                                                    void openRecentFile(recentFile.path, recentFile.source, hit.line_number);
                                                                }}
                                                            >
                                                                <span class="shrink-0 select-none tabular-nums text-xs text-sidebar-foreground/40">{hit.line_number}</span>
                                                                <span class="min-w-0 truncate font-mono text-xs leading-relaxed {isActiveFile ? 'text-black/70 dark:text-white/70' : 'text-sidebar-foreground/65'}">
                                                                    {#each splitTextByTerms(getLineExcerpt(hit.line_text, searchTerms), searchTerms) as fragment}
                                                                        {#if fragment.isMatch}<mark class="bg-[var(--selection-match-bg)] text-inherit rounded-sm px-0.5 ring-1 ring-[var(--selection-match-border)]">{fragment.text}</mark>{:else}{fragment.text}{/if}
                                                                    {/each}
                                                                </span>
                                                            </button>
                                                        {/each}
                                                    </div>
                                                {/if}
                                            </div>
                                        </Item.Root>

                                        <ContextMenu.Content class="border-sidebar-border bg-sidebar text-sidebar-foreground">
                                            <ContextMenu.Item
                                                onclick={() => {
                                                    void openRecentFile(recentFile.path, recentFile.source);
                                                }}
                                            >
                                                <Files class="size-4" />
                                                <span>Open</span>
                                            </ContextMenu.Item>
                                            <ContextMenu.Item
                                                onclick={() => {
                                                    void handleRevealRecentFile(recentFile.path);
                                                }}
                                            >
                                                <FolderOpen class="size-4" />
                                                <span>{getRevealInFileManagerLabel()}</span>
                                            </ContextMenu.Item>
                                            <ContextMenu.Item
                                                onclick={() => {
                                                    void handleCopyRecentFilePath(recentFile.path);
                                                }}
                                            >
                                                <Copy class="size-4" />
                                                <span>Copy Path</span>
                                            </ContextMenu.Item>
                                            {#if recentFile.source === "local"}
                                                <ContextMenu.Separator />
                                                <ContextMenu.Item
                                                    onclick={() => {
                                                        void handleDuplicateLocalFileAsSlate(recentFile);
                                                    }}
                                                >
                                                    <CopyPlus class="size-4" />
                                                    <span>Duplicate as Slate</span>
                                                </ContextMenu.Item>
                                            {/if}
                                            {#if recentFile.source === "slates"}
                                                <ContextMenu.Separator />
                                                <ContextMenu.Item
                                                    onclick={() => {
                                                        void handleDuplicateRecentFile(recentFile);
                                                    }}
                                                >
                                                    <CopyPlus class="size-4" />
                                                    <span>Duplicate</span>
                                                </ContextMenu.Item>
                                                <ContextMenu.Item
                                                    onclick={() => {
                                                        openRenameFileDialog(recentFile);
                                                    }}
                                                >
                                                    <Pencil class="size-4" />
                                                    <span>Rename</span>
                                                </ContextMenu.Item>
                                                <ContextMenu.Item
                                                    class="text-destructive focus:text-destructive"
                                                    onclick={() => {
                                                        openDeleteFileDialog(recentFile);
                                                    }}
                                                >
                                                    <Trash2 class="size-4" />
                                                    <span>Delete</span>
                                                </ContextMenu.Item>
                                            {/if}
                                        </ContextMenu.Content>
                                    </ContextMenu.Root>
                                {/each}
                            </Item.Group>
                        </section>
                    {/each}
                </Sidebar.GroupContent>
            {/if}
        </Sidebar.Group>
    </div>
</div>
