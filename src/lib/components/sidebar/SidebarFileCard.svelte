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
    import { invoke } from "@tauri-apps/api/core";
    import { toast } from "$lib/components/ui/sonner";
    import type { LanguageIcon } from "$lib/editor/config/languageIconMap";
    import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
    import * as Item from "$lib/components/ui/item/index.js";
    import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";
    import { getPlatformOsType } from "$lib/state/platform.svelte";
    import { openRenameFileDialog } from "$lib/state/appDialogs.svelte";
    import {
        isSearchResult,
        formatSize,
        formatTimestamp,
        formatTimestampFull,
        getRecencyTimestamp,
        type LibraryFileRecord,
    } from "$lib/files/sidebarUtils";
    import { requestDeleteFile, type RecentFileRecord, type RecentFileSource } from "$lib/files/recentFiles";
    import Files from "~icons/lucide/files";
    import FolderOpen from "~icons/lucide/folder-open";
    import Copy from "~icons/lucide/copy";
    import CopyPlus from "~icons/lucide/copy-plus";
    import Pencil from "~icons/lucide/pencil";
    import Trash2 from "~icons/lucide/trash-2";
    import LucideUnlink2 from '~icons/lucide/unlink-2';
    import Ellipsis from "~icons/lucide/ellipsis";
    import LucideHardDrive from "~icons/lucide/hard-drive";

    interface Props {
        recentFile: LibraryFileRecord;
        /** Whether the local-file badge should be shown in the current tab. */
        showLocalBadge?: boolean;
        isActive: boolean;
        /** Keyboard-navigated highlight (ArrowUp/Down from the search input). */
        isHighlighted?: boolean;
        onOpen: (path: string, source: RecentFileSource, lineNumber?: number) => void;
        /** Called when the pointer enters this card — lets the parent sync highlightedIndex. */
        onHover?: () => void;
        /** Provided only for slate files — omit for local files. */
        onDuplicate?: (file: RecentFileRecord) => void;
        /** Provided only for local files — omit for slate files. */
        onDuplicateAsSlate?: (file: RecentFileRecord) => void;
        /** Provided only for local files — unlink from sidebar without deleting from disk. */
        onUnlink?: (file: RecentFileRecord) => void;
    }

    const {
        recentFile,
        showLocalBadge = true,
        isActive,
        isHighlighted = false,
        onOpen,
        onHover,
        onDuplicate,
        onDuplicateAsSlate,
        onUnlink,
    }: Props = $props();

    // ---------------------------------------------------------------------------
    // Language / display helpers
    // ---------------------------------------------------------------------------

    const fileLangMeta = $derived(languageMetaByValue.get(recentFile.language ?? "text"));
    const FileIcon = $derived(fileLangMeta?.icon ?? null);
    const fileLanguageLabel = $derived(fileLangMeta?.label ?? "Text");
    const fileSize = $derived(formatSize(recentFile.size_bytes));
    const searchResult = $derived(isSearchResult(recentFile) ? recentFile : null);

    /** Cap visible matched lines per card to keep the DOM lightweight when
     *  results arrive.  The backend may return up to 50 per file. */
    const MAX_VISIBLE_MATCHED_LINES = 5;
    const visibleMatchedLines = $derived(
        searchResult ? searchResult.matched_lines.slice(0, MAX_VISIBLE_MATCHED_LINES) : [],
    );
    const hiddenMatchedLinesCount = $derived(
        searchResult ? Math.max(0, searchResult.matched_lines.length - MAX_VISIBLE_MATCHED_LINES) : 0,
    );

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
            await invoke("reveal_document", {
                documentId: recentFile.document_id,
                documentGeneration: recentFile.document_generation,
            });
        } catch {
            toast.error("Failed to open containing folder");
        }
    }
</script>

<ContextMenu.Root>
    <Item.Root
        variant="outline"
        size="sm"
        data-sidebar-highlighted={isHighlighted || undefined}
        data-card-path={recentFile.path}
        class="border-0 p-0 shadow-none [transform:translateZ(0)] {isActive ? 'ring-1 ring-inset ring-sidebar-ring bg-sidebar-foreground/[0.03]' : 'ring-1 ring-inset ring-sidebar-border/65 bg-sidebar/35'}"
    >
        <div role="presentation" class="w-full overflow-hidden rounded-[inherit]" onmouseenter={() => onHover?.()}>
            <ContextMenu.Trigger>
                {#snippet child({ props })}
                    <div
                        {...props}
                        class="group relative transition-colors {isActive ? 'bg-sidebar-foreground/[0.04] text-sidebar-foreground' : isHighlighted ? 'bg-sidebar-accent/70 text-sidebar-accent-foreground' : 'data-[state=open]:bg-sidebar-accent/70 data-[state=open]:text-sidebar-accent-foreground'}"
                    >
                        <button
                            type="button"
                            class="flex w-full min-w-0 items-start gap-3 px-3.5 py-3 pr-9 text-left outline-none"
                            onclick={() => onOpen(recentFile.path, recentFile.source)}
                        >
                        <Item.Media
                            variant="icon"
                            title={fileLanguageLabel}
                            class="relative mt-0.5 {isActive ? 'border-sidebar-ring/40 bg-sidebar-foreground/[0.04] text-sidebar-foreground' : isHighlighted ? 'border-sidebar-background/60 bg-sidebar/80 text-sidebar-accent-foreground' : 'border-sidebar-border/70 bg-sidebar-accent/45 text-muted-foreground group-data-[state=open]:border-sidebar-background/60 group-data-[state=open]:bg-sidebar/80 group-data-[state=open]:text-sidebar-accent-foreground'}"
                        >
                            {#if FileIcon}
                                <FileIcon class="size-4.5" />
                            {:else}
                                <Files class="size-4.5" />
                            {/if}
                            {#if recentFile.source === "local" && showLocalBadge}
                                <!-- Corner marker for local files. -->
                                <span
                                    aria-hidden="true"
                                    class="pointer-events-none absolute -bottom-0.5 -right-0.5 z-10 flex size-3.5 items-center justify-center rounded-sm {isActive ? 'file-icon-badge-active' : isHighlighted ? 'file-icon-badge-emphasis' : 'file-icon-badge-inactive'}"
                                >
                                    <LucideHardDrive class="!size-3" />
                                </span>
                            {/if}
                        </Item.Media>

                        <Item.Content class="min-w-0 gap-2.5">
                            <div class="flex items-start justify-between gap-3">
                                <div class="min-w-0 flex-1">
                                    <Item.Title title={recentFile.path} class="truncate text-sm leading-tight {isActive ? 'text-black dark:text-white' : isHighlighted ? 'text-sidebar-accent-foreground' : 'text-sidebar-foreground group-data-[state=open]:text-sidebar-accent-foreground'}">
                                        {#if searchResult && searchResult.filename_fragments.length > 0}
                                            {#each searchResult.filename_fragments as fragment}
                                                {#if fragment.is_match}<mark class="bg-[var(--selection-match-bg)] text-inherit rounded-sm px-0.5 ring-1 ring-inset ring-[var(--selection-match-border)]">{fragment.text}</mark>{:else}{fragment.text}{/if}
                                            {/each}
                                        {:else}
                                            {recentFile.file_name}
                                        {/if}
                                    </Item.Title>
                                </div>

                                {#if searchResult && searchResult.match_count > 0}
                                    <Item.Actions class="pt-0.5">
                                        <span class="shrink-0 whitespace-nowrap text-xs tabular-nums text-muted-foreground">
                                            {searchResult.match_count} {searchResult.match_count === 1 ? "hit" : "hits"}
                                        </span>
                                    </Item.Actions>
                                {/if}
                            </div>

                            <div class="flex min-w-0 flex-nowrap items-center gap-2 overflow-hidden text-xs {isActive ? 'text-muted-foreground' : isHighlighted ? 'text-sidebar-accent-foreground' : 'text-muted-foreground group-data-[state=open]:text-sidebar-accent-foreground'}">
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
                                        class="absolute right-1.5 top-1/2 -translate-y-1/2 flex size-6 items-center justify-center rounded transition-opacity data-[state=open]:opacity-100 hover:bg-sidebar-foreground/10 text-sidebar-foreground {isActive || isHighlighted ? 'opacity-100' : 'opacity-0 group-data-[state=open]:opacity-100'}"
                                    >
                                        <Ellipsis class="size-4" />
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
                                        {#if onUnlink}
                                            <DropdownMenuPrimitive.Item
                                                class={ddItemClass}
                                                onclick={() => { onUnlink(recentFile as RecentFileRecord); }}
                                            >
                                                <LucideUnlink2 class="size-4" />
                                                <span>Unlink</span>
                                            </DropdownMenuPrimitive.Item>
                                        {/if}
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
                                            onclick={() => { requestDeleteFile(recentFile as RecentFileRecord); }}
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
                    {#each visibleMatchedLines as hit (hit.line_number)}
                        <button
                            type="button"
                            class="flex w-full min-w-0 items-baseline gap-2.5 rounded px-1.5 py-1 text-left transition-colors hover:bg-sidebar-accent/50"
                            title="Go to line {hit.line_number}"
                            onclick={() => onOpen(recentFile.path, recentFile.source, hit.line_number)}
                        >
                            <span class="shrink-0 select-none tabular-nums text-xs text-disabled-foreground">{hit.line_number}</span>
                            <span class="min-w-0 truncate font-mono text-[0.8rem] leading-relaxed text-muted-foreground">
                                {#each hit.fragments as fragment}
                                    {#if fragment.is_match}<mark class="bg-[var(--selection-match-bg)] text-inherit rounded-sm px-0.5 ring-1 ring-inset ring-[var(--selection-match-border)]">{fragment.text}</mark>{:else}{fragment.text}{/if}
                                {/each}
                            </span>
                        </button>
                    {/each}
                    {#if hiddenMatchedLinesCount > 0}
                        <div
                            role="button"
                            tabindex="-1"
                            class="cursor-pointer px-1.5 py-1 text-xs text-disabled-foreground"
                            onclick={() => onOpen(recentFile.path, recentFile.source)}
                            onkeydown={(e) => { if (e.key === 'Enter') onOpen(recentFile.path, recentFile.source); }}
                        >
                            +{hiddenMatchedLinesCount} more line{hiddenMatchedLinesCount === 1 ? '' : 's'}
                        </div>
                    {/if}
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
            {#if onUnlink}
                <ContextMenu.Item onclick={() => onUnlink(recentFile as RecentFileRecord)}>
                    <LucideUnlink2 class="size-4" />
                    <span>Unlink</span>
                </ContextMenu.Item>
            {/if}
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
                onclick={() => requestDeleteFile(recentFile as RecentFileRecord)}
            >
                <Trash2 class="size-4" />
                <span>Delete</span>
            </ContextMenu.Item>
        {/if}
    </ContextMenu.Content>
</ContextMenu.Root>

<style>
    /*
     * The media background uses translucent layers. These opaque equivalents
     * match the final rendered surface while covering the media border behind
     * the transparent hard-drive SVG.
     */
    .file-icon-badge-active {
        background-color: color-mix(in srgb, var(--sidebar-foreground) 11%, var(--sidebar));
    }

    .file-icon-badge-inactive {
        background-color: color-mix(in srgb, var(--sidebar-accent) 45%, var(--sidebar));
    }

    .file-icon-badge-emphasis {
        background-color: color-mix(in srgb, var(--sidebar-accent) 14%, var(--sidebar));
    }
</style>
