<script lang="ts">
    import * as Sidebar from "$lib/components/ui/sidebar/index.js";
    import * as Item from "$lib/components/ui/item/index.js";
    import Files from "~icons/lucide/files";
    import SidebarFileCard from "./SidebarFileCard.svelte";
    import type { LibraryFileRecord, RecentFileSection } from "$lib/files/sidebarUtils";
    import type { RecentFileRecord, RecentFileSource } from "$lib/files/recentFiles";
    import { hotkey, type HotkeyBinding } from "$lib/hotkeys";

    interface Props {
        sections: RecentFileSection[];
        /** True while the search query is non-empty (stable boolean, not the raw query string). */
        isSearchMode: boolean;
        isLoading: boolean;
        isSearchLoading: boolean;
        /** All visible results across both browse and search modes (used for empty-state detection). */
        activeResults: LibraryFileRecord[];
        loadError: string;
        /** Path of the currently keyboard-highlighted result. */
        highlightedPath: string | undefined;
        /** Path of the file currently being opened (pending navigation). */
        pendingOpenFilePath: string | undefined;
        /** Path of the file currently shown in the editor. */
        currentFilePath: string | undefined;
        /** DOM ref propagated back to parent so it can call scrollTo. */
        scrollContainer?: HTMLDivElement | null;
        onOpen: (path: string, source: RecentFileSource, lineNumber?: number) => void;
        /** Called when the pointer enters a card — parent syncs highlightedIndex without scrolling. */
        onHighlight: (path: string) => void;
        /** TanStack hotkey bindings for ArrowUp/Down/Enter navigation, applied to the scroll container. Pass `navigator.listHotkeys`. */
        listHotkeys?: HotkeyBinding[];
        onDuplicate: (file: RecentFileRecord) => void;
        onDuplicateAsSlate: (file: RecentFileRecord) => void;
        onUnlink: (file: RecentFileRecord) => void;
    }

    let {
        sections,
        isSearchMode,
        isLoading,
        isSearchLoading,
        activeResults,
        loadError,
        highlightedPath,
        pendingOpenFilePath,
        currentFilePath,
        scrollContainer = $bindable(null),
        onOpen,
        onHighlight,
        listHotkeys = [],
        onDuplicate,
        onDuplicateAsSlate,
        onUnlink,
    }: Props = $props();

    /**
     * Determines whether a file card should render in the active/selected state.
     * Prefer pendingOpenFilePath while a navigation is in-flight to avoid a
     * flicker between "pending" and "current" during the transition.
     */
    function isActive(recentFile: LibraryFileRecord): boolean {
        if (pendingOpenFilePath !== undefined) {
            return recentFile.path === pendingOpenFilePath;
        }
        return !!currentFilePath && currentFilePath === recentFile.path;
    }
</script>

<div bind:this={scrollContainer} class="flex-1 min-h-0 overflow-auto p-2" use:hotkey={listHotkeys}>
    <Sidebar.Group class="gap-2 p-0">
        {#if loadError}
            <div class="rounded-lg border border-destructive/30 bg-destructive/8 px-3 py-2 text-sm text-destructive">
                {loadError}
            </div>

        {:else if (!isSearchMode && isLoading && activeResults.length === 0)
            || (isSearchMode && isSearchLoading && activeResults.length === 0)}
            <!-- Skeleton — shown only on the initial load before any results arrive. -->
            <div class="space-y-2 px-1 pt-1">
                {#each Array.from({ length: 5 }) as _, index (index)}
                    <div class="rounded-lg border border-sidebar-border/60 bg-sidebar-accent/40 px-3 py-3 animate-pulse">
                        <div class="h-3 w-3/5 rounded bg-sidebar-foreground/10"></div>
                        <div class="mt-2 h-2.5 w-4/5 rounded bg-sidebar-foreground/10"></div>
                    </div>
                {/each}
            </div>

        {:else if activeResults.length === 0}
            <div class="flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-sidebar-border/70 px-4 py-10 text-center text-sm text-muted-foreground">
                <Files class="size-5 text-muted-foreground" />
                <div>
                    {isSearchMode ? "No files match this search." : "No recent files yet."}
                </div>
            </div>

        {:else}
            <Sidebar.GroupContent class="space-y-4 px-1 pb-2">
                {#each sections as section (section.key)}
                    <section class="space-y-2">
                        {#if section.label}
                            <div class="flex items-center gap-3 px-2 pt-1">
                                <span class="truncate whitespace-nowrap text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                                    {section.label}
                                </span>
                                <div class="h-px flex-1 bg-sidebar-border/70"></div>
                                <span class="shrink-0 whitespace-nowrap text-xs text-muted-foreground">
                                    {section.items.length}
                                </span>
                            </div>
                        {/if}

                        <Item.Group class="gap-2">
                            {#each section.items as recentFile (recentFile.path)}
                                <SidebarFileCard
                                    {recentFile}
                                    isActive={isActive(recentFile)}
                                    isHighlighted={recentFile.path === highlightedPath}
                                    {onOpen}
                                    onHover={() => onHighlight(recentFile.path)}
                                    {onDuplicate}
                                    {onDuplicateAsSlate}
                                    {onUnlink}
                                />
                            {/each}
                        </Item.Group>
                    </section>
                {/each}
            </Sidebar.GroupContent>
        {/if}
    </Sidebar.Group>
</div>
