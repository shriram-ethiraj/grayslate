<script module lang="ts">
    /**
     * Shared lookup — built once at module load, not per card instance.
     * Maps language value → metadata (icon, label, token).
     */
    import { languages } from "$lib/editor/config/languageIconMap";
    const languageMetaByValue = new Map(languages.map((l) => [l.value, l] as const));
</script>

<script lang="ts">
    import { writeText } from "@tauri-apps/plugin-clipboard-manager";
    import { revealItemInDir } from "@tauri-apps/plugin-opener";
    import { toast } from "$lib/components/ui/sonner";
    import type { LanguageIcon } from "$lib/editor/config/languageIconMap";
    import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
    import * as Item from "$lib/components/ui/item/index.js";
    import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";
    import { getPlatformOsType } from "$lib/state/platform.svelte";
    import { openDeleteFileDialog, openRenameFileDialog } from "$lib/state/appDialogs.svelte";
    import {
        getLineExcerpt,
        isSearchResult,
        splitTextByTerms,
        formatSize,
        formatTimestamp,
        formatTimestampFull,
        getRecencyTimestamp,
        type LibraryFileRecord,
    } from "$lib/files/sidebarUtils";
    import type { RecentFileRecord, RecentFileSource } from "$lib/files/recentFiles";
    import Files from "~icons/lucide/files";
    import FolderOpen from "~icons/lucide/folder-open";
    import Copy from "~icons/lucide/copy";
    import CopyPlus from "~icons/lucide/copy-plus";
    import Pencil from "~icons/lucide/pencil";
    import Trash2 from "~icons/lucide/trash-2";
    import FileWarning from "~icons/lucide/file-warning";
    import Ellipsis from "~icons/lucide/ellipsis";

    interface Props {
        recentFile: LibraryFileRecord;
        isActive: boolean;
        searchTerms: string[];
        onOpen: (path: string, source: RecentFileSource, lineNumber?: number) => void;
        /** Provided only for slate files — omit for local files. */
        onDuplicate?: (file: RecentFileRecord) => void;
        /** Provided only for local files — omit for slate files. */
        onDuplicateAsSlate?: (file: RecentFileRecord) => void;
    }

    const { recentFile, isActive, searchTerms, onOpen, onDuplicate, onDuplicateAsSlate }: Props = $props();

    // ---------------------------------------------------------------------------
    // Language / display helpers
    // ---------------------------------------------------------------------------

    const FileIcon = $derived(languageMetaByValue.get(recentFile.language ?? "text")?.icon ?? null);
    const fileSize = $derived(formatSize(recentFile.size_bytes));
    const searchResult = $derived(isSearchResult(recentFile) ? recentFile : null);

    /** Shared CSS class for dropdown menu items. */
    const ddItemClass = "data-highlighted:bg-accent data-highlighted:text-accent-foreground relative flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-hidden select-none [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4";
    const ddSepClass = "bg-border -mx-1 my-1 h-px";

    // ---------------------------------------------------------------------------
    // Context menu actions (self-contained — no parent state needed)
    // ---------------------------------------------------------------------------

    function getRevealLabel(): string {
        switch (getPlatformOsType()) {
            case "macos": return "Show in Finder";
            case "linux": return "Show in File Manager";
            default: return "Show in Explorer";
        }
    }

    async function handleCopyPath(): Promise<void> {
        try {
            await writeText(recentFile.path);
        } catch {
            toast.error("Failed to copy path");
        }
    }

    async function handleReveal(): Promise<void> {
        try {
            await revealItemInDir(recentFile.path);
        } catch {
            toast.error("Failed to open containing folder");
        }
    }
</script>

<ContextMenu.Root>
    <Item.Root
        variant="outline"
        size="sm"
        class="border-0 p-0 shadow-none [transform:translateZ(0)] {isActive ? 'ring-1 ring-inset ring-sidebar-ring bg-sidebar-foreground/[0.03]' : 'ring-1 ring-inset ring-sidebar-border/65 bg-sidebar/35'}"
    >
        <div class="w-full overflow-hidden rounded-[inherit]">
            <ContextMenu.Trigger>
                {#snippet child({ props })}
                    <div
                        {...props}
                        class="group relative transition-colors {isActive ? 'bg-sidebar-foreground/[0.04] text-sidebar-foreground' : 'hover:bg-sidebar-accent/70 hover:text-sidebar-accent-foreground data-[state=open]:bg-sidebar-accent/70 data-[state=open]:text-sidebar-accent-foreground'}"
                    >
                        <button
                            type="button"
                            class="flex w-full min-w-0 items-start gap-3 px-3.5 py-3 pr-9 text-left outline-none"
                            title={recentFile.path}
                            onclick={() => onOpen(recentFile.path, recentFile.source)}
                        >
                        <Item.Media
                            variant="icon"
                            class="mt-0.5 {isActive ? 'border-sidebar-ring/40 bg-sidebar-foreground/[0.04] text-sidebar-foreground' : 'border-sidebar-border/70 bg-sidebar-accent/45 text-sidebar-foreground/80 group-hover:border-sidebar-background/60 group-hover:bg-sidebar/80 group-hover:text-sidebar-accent-foreground group-data-[state=open]:border-sidebar-background/60 group-data-[state=open]:bg-sidebar/80 group-data-[state=open]:text-sidebar-accent-foreground'}"
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
                                    <Item.Title class="truncate text-sm leading-tight {isActive ? 'text-black dark:text-white' : 'text-sidebar-foreground group-hover:text-sidebar-accent-foreground group-data-[state=open]:text-sidebar-accent-foreground'}">
                                        {#if searchTerms.length > 0}
                                            {#each splitTextByTerms(recentFile.file_name, searchTerms) as fragment}
                                                {#if fragment.isMatch}<mark class="bg-[var(--selection-match-bg)] text-inherit rounded-sm px-0.5 ring-1 ring-[var(--selection-match-border)]">{fragment.text}</mark>{:else}{fragment.text}{/if}
                                            {/each}
                                        {:else}
                                            {recentFile.file_name}
                                        {/if}
                                    </Item.Title>
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
                                        <span class="shrink-0 whitespace-nowrap text-xs tabular-nums {isActive ? 'text-black/60 dark:text-white/65' : 'text-sidebar-foreground/50'}">
                                            {searchResult.match_count} {searchResult.match_count === 1 ? "hit" : "hits"}
                                        </span>
                                    </Item.Actions>
                                {/if}
                            </div>

                            <div class="flex min-w-0 flex-nowrap items-center gap-2 overflow-hidden text-xs {isActive ? 'text-black/70 dark:text-white/74' : 'text-sidebar-foreground/55 group-hover:text-sidebar-accent-foreground/72 group-data-[state=open]:text-sidebar-accent-foreground/72'}">
                                {#if fileSize}
                                    <span class="truncate whitespace-nowrap">{fileSize}</span>
                                    <span aria-hidden="true" class="shrink-0">•</span>
                                {/if}
                                <span class="truncate whitespace-nowrap" title={formatTimestampFull(getRecencyTimestamp(recentFile))}>
                                    {formatTimestamp(getRecencyTimestamp(recentFile))}
                                </span>
                            </div>
                        </Item.Content>
                        </button>

                        <!-- Three-dot options button (visible on hover / when active) -->
                        <DropdownMenuPrimitive.Root>
                            <DropdownMenuPrimitive.Trigger>
                                {#snippet child({ props: dotProps })}
                                    <button
                                        {...dotProps}
                                        type="button"
                                        title="File options"
                                        class="absolute right-1.5 top-1/2 -translate-y-1/2 flex size-6 items-center justify-center rounded transition-opacity data-[state=open]:opacity-100 hover:bg-sidebar-foreground/10 text-sidebar-foreground/50 hover:text-sidebar-foreground {isActive ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'}"
                                    >
                                        <Ellipsis class="size-3.5" />
                                    </button>
                                {/snippet}
                            </DropdownMenuPrimitive.Trigger>
                            <DropdownMenuPrimitive.Portal>
                                <DropdownMenuPrimitive.Content
                                    align="end"
                                    class="z-50 min-w-[8rem] overflow-hidden rounded-md border border-sidebar-border bg-sidebar p-1 text-sidebar-foreground shadow-md data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95"
                                >
                                    <DropdownMenuPrimitive.Item
                                        class={ddItemClass}
                                        onclick={() => { onOpen(recentFile.path, recentFile.source); }}
                                    >
                                        <Files class="size-4" />
                                        <span>Open</span>
                                    </DropdownMenuPrimitive.Item>
                                    <DropdownMenuPrimitive.Item
                                        class={ddItemClass}
                                        onclick={handleReveal}
                                    >
                                        <FolderOpen class="size-4" />
                                        <span>{getRevealLabel()}</span>
                                    </DropdownMenuPrimitive.Item>
                                    <DropdownMenuPrimitive.Item
                                        class={ddItemClass}
                                        onclick={handleCopyPath}
                                    >
                                        <Copy class="size-4" />
                                        <span>Copy Path</span>
                                    </DropdownMenuPrimitive.Item>
                                    {#if recentFile.source === "local" && onDuplicateAsSlate}
                                        <DropdownMenuPrimitive.Separator class={ddSepClass} />
                                        <DropdownMenuPrimitive.Item
                                            class={ddItemClass}
                                            onclick={() => { onDuplicateAsSlate(recentFile as RecentFileRecord); }}
                                        >
                                            <CopyPlus class="size-4" />
                                            <span>Duplicate as Slate</span>
                                        </DropdownMenuPrimitive.Item>
                                    {/if}
                                    {#if recentFile.source === "slates" && onDuplicate}
                                        <DropdownMenuPrimitive.Separator class={ddSepClass} />
                                        <DropdownMenuPrimitive.Item
                                            class={ddItemClass}
                                            onclick={() => { onDuplicate(recentFile as RecentFileRecord); }}
                                        >
                                            <CopyPlus class="size-4" />
                                            <span>Duplicate</span>
                                        </DropdownMenuPrimitive.Item>
                                        <DropdownMenuPrimitive.Item
                                            class={ddItemClass}
                                            onclick={() => { openRenameFileDialog(recentFile as RecentFileRecord); }}
                                        >
                                            <Pencil class="size-4" />
                                            <span>Rename</span>
                                        </DropdownMenuPrimitive.Item>
                                        <DropdownMenuPrimitive.Item
                                            class="{ddItemClass} text-destructive data-highlighted:bg-destructive/10 dark:data-highlighted:bg-destructive/20 data-highlighted:text-destructive"
                                            onclick={() => { openDeleteFileDialog(recentFile as RecentFileRecord); }}
                                        >
                                            <Trash2 class="size-4" />
                                            <span>Delete</span>
                                        </DropdownMenuPrimitive.Item>
                                    {/if}
                                </DropdownMenuPrimitive.Content>
                            </DropdownMenuPrimitive.Portal>
                        </DropdownMenuPrimitive.Root>
                    </div>
                {/snippet}
            </ContextMenu.Trigger>

            {#if searchResult && searchResult.matched_lines.length > 0}
                <div class="border-t border-sidebar-border/40 px-3 py-1.5">
                    {#each searchResult.matched_lines as hit (hit.line_number)}
                        <button
                            type="button"
                            class="flex w-full min-w-0 items-baseline gap-2.5 rounded px-1.5 py-1 text-left transition-colors hover:bg-sidebar-accent/50"
                            title="Go to line {hit.line_number}"
                            onclick={() => onOpen(recentFile.path, recentFile.source, hit.line_number)}
                        >
                            <span class="shrink-0 select-none tabular-nums text-xs text-sidebar-foreground/40">{hit.line_number}</span>
                            <span class="min-w-0 truncate font-mono text-xs leading-relaxed {isActive ? 'text-black/70 dark:text-white/70' : 'text-sidebar-foreground/65'}">
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
        <ContextMenu.Item onclick={() => onOpen(recentFile.path, recentFile.source)}>
            <Files class="size-4" />
            <span>Open</span>
        </ContextMenu.Item>
        <ContextMenu.Item onclick={handleReveal}>
            <FolderOpen class="size-4" />
            <span>{getRevealLabel()}</span>
        </ContextMenu.Item>
        <ContextMenu.Item onclick={handleCopyPath}>
            <Copy class="size-4" />
            <span>Copy Path</span>
        </ContextMenu.Item>

        {#if recentFile.source === "local" && onDuplicateAsSlate}
            <ContextMenu.Separator />
            <ContextMenu.Item onclick={() => onDuplicateAsSlate(recentFile as RecentFileRecord)}>
                <CopyPlus class="size-4" />
                <span>Duplicate as Slate</span>
            </ContextMenu.Item>
        {/if}

        {#if recentFile.source === "slates" && onDuplicate}
            <ContextMenu.Separator />
            <ContextMenu.Item onclick={() => onDuplicate(recentFile as RecentFileRecord)}>
                <CopyPlus class="size-4" />
                <span>Duplicate</span>
            </ContextMenu.Item>
            <ContextMenu.Item onclick={() => openRenameFileDialog(recentFile as RecentFileRecord)}>
                <Pencil class="size-4" />
                <span>Rename</span>
            </ContextMenu.Item>
            <ContextMenu.Item
                class="text-destructive focus:text-destructive"
                onclick={() => openDeleteFileDialog(recentFile as RecentFileRecord)}
            >
                <Trash2 class="size-4" />
                <span>Delete</span>
            </ContextMenu.Item>
        {/if}
    </ContextMenu.Content>
</ContextMenu.Root>
