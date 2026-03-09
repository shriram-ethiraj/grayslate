<script lang="ts">
    import { tick } from "svelte";
    import type { LanguageIcon } from "$lib/editor/config/supportedLanguages";
    import { languages } from "$lib/editor/config/supportedLanguages";
    import { languageDetector } from "$lib/editor/core/languageDetector";
    import * as Sidebar from "$lib/components/ui/sidebar/index.js";
    import * as Item from "$lib/components/ui/item/index.js";
    import * as Select from "$lib/components/ui/select/index.js";
    import * as Tabs from "$lib/components/ui/tabs/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import Input from "$lib/components/ui/input/input.svelte";
    import {
        getRecentFiles,
        OPEN_FILE_PATH_EVENT,
        RECENT_FILES_UPDATED_EVENT,
        type OpenFilePathPayload,
        type RecentFileRecord,
        type RecentFileSource,
    } from "$lib/files/recentFiles";
    import Search from "~icons/lucide/search";
    import RefreshCcw from "~icons/lucide/refresh-ccw";
    import Files from "~icons/lucide/files";
    import Clock3 from "~icons/lucide/clock-3";
    import History from "~icons/lucide/history";
    import ArrowDownAZ from "~icons/lucide/arrow-down-a-z";
    import ArrowUpZA from "~icons/lucide/arrow-up-z-a";
    import ArrowDownWideNarrow from "~icons/lucide/arrow-down-wide-narrow";
    import ArrowUpNarrowWide from "~icons/lucide/arrow-up-narrow-wide";
    import NotebookText from "~icons/lucide/notebook-text";
    import ExternalLink from "~icons/lucide/external-link";
    import FileWarning from "~icons/lucide/file-warning";

    type FilterMode = "unified" | RecentFileSource;
    type SortMode =
        | "last-modified"
        | "oldest"
        | "name-asc"
        | "name-desc"
        | "size-desc"
        | "size-asc";
    type RecencyBucket = "today" | "this-week" | "older";

    interface RecentFileSection {
        key: RecencyBucket | "all";
        label: string;
        items: RecentFileRecord[];
    }

    const RECENT_FILES_LIMIT = 120;
    const textCollator = new Intl.Collator(undefined, {
        numeric: true,
        sensitivity: "base",
    });
    const languageMetaByValue = new Map(languages.map((language) => [language.value, language] as const));
    const recencySectionOrder: Record<Extract<SortMode, "last-modified" | "oldest">, RecencyBucket[]> = {
        "last-modified": ["today", "this-week", "older"],
        oldest: ["older", "this-week", "today"],
    };
    const recencySectionLabels: Record<RecencyBucket, string> = {
        today: "Today",
        "this-week": "This week",
        older: "Older",
    };
    const languageTypeTokens: Partial<Record<string, string>> = {
        text: "TXT",
        markdown: "MD",
        javascript: "JS",
        typescript: "TS",
        python: "PY",
        powershell: "PS1",
        shell: "SH",
        dockerfile: "DOCKER",
    };

    let query = $state("");
    let filterMode = $state<FilterMode>("unified");
    let sortMode = $state<SortMode>("last-modified");
    let recentFiles = $state<RecentFileRecord[]>([]);
    let isLoading = $state(false);
    let loadError = $state("");
    let requestVersion = 0;
    let searchInput = $state<HTMLInputElement | null>(null);

    const sidebar = Sidebar.useSidebar();

    const normalizedQuery = $derived(query.trim().toLowerCase());

    const visibleRecentFiles = $derived.by(() => {
        const filteredRecentFiles = recentFiles.filter((recentFile) => {
            if (filterMode !== "unified" && recentFile.source !== filterMode) {
                return false;
            }

            if (normalizedQuery.length === 0) {
                return true;
            }

            const haystack = [
                recentFile.file_name,
                recentFile.path,
                recentFile.extension ?? "",
                getRecentFileTypeToken(recentFile),
            ]
                .join(" ")
                .toLowerCase();

            return haystack.includes(normalizedQuery);
        });

        filteredRecentFiles.sort((left, right) => compareRecentFiles(left, right, sortMode));
        return filteredRecentFiles;
    });

    const recentFileSections = $derived.by(() => {
        if (sortMode !== "last-modified" && sortMode !== "oldest") {
            return [{
                key: "all",
                label: "",
                items: visibleRecentFiles,
            }] satisfies RecentFileSection[];
        }

        return buildRecencySections(visibleRecentFiles, sortMode);
    });

    const filterOptions: Array<{
        value: FilterMode;
        label: string;
        title: string;
        icon: typeof Files;
    }> = [
        {
            value: "unified",
            label: "All",
            title: "Show internal and external recent files",
            icon: Files,
        },
        {
            value: "internal",
            label: "Slates",
            title: "Show Grayslate documents only",
            icon: NotebookText,
        },
        {
            value: "external",
            label: "External",
            title: "Show previously opened external files only",
            icon: ExternalLink,
        },
    ];

    const sortOptions: Array<{
        value: SortMode;
        label: string;
        icon: typeof Search;
    }> = [
        { value: "last-modified", label: "Last modified", icon: Clock3 },
        { value: "oldest", label: "Oldest first", icon: History },
        { value: "name-asc", label: "Name (A to Z)", icon: ArrowDownAZ },
        { value: "name-desc", label: "Name (Z to A)", icon: ArrowUpZA },
        { value: "size-desc", label: "Largest first", icon: ArrowDownWideNarrow },
        { value: "size-asc", label: "Smallest first", icon: ArrowUpNarrowWide },
    ];

    const activeSortOption = $derived(
        sortOptions.find((option) => option.value === sortMode) ?? sortOptions[0],
    );

    const relativeTimeFormatter = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

    function formatTimestamp(value: number | null): string {
        if (!value) {
            return "Unknown";
        }

        const deltaMs = value - Date.now();
        const deltaMinutes = Math.round(deltaMs / 60_000);

        if (Math.abs(deltaMinutes) < 60) {
            return relativeTimeFormatter.format(deltaMinutes, "minute");
        }

        const deltaHours = Math.round(deltaMinutes / 60);
        if (Math.abs(deltaHours) < 48) {
            return relativeTimeFormatter.format(deltaHours, "hour");
        }

        const deltaDays = Math.round(deltaHours / 24);
        if (Math.abs(deltaDays) < 30) {
            return relativeTimeFormatter.format(deltaDays, "day");
        }

        const deltaMonths = Math.round(deltaDays / 30);
        if (Math.abs(deltaMonths) < 12) {
            return relativeTimeFormatter.format(deltaMonths, "month");
        }

        return relativeTimeFormatter.format(Math.round(deltaMonths / 12), "year");
    }

    function formatSize(value: number | null): string {
        if (!value || value <= 0) {
            return "";
        }

        const units = ["B", "KB", "MB", "GB"];
        let size = value;
        let unitIndex = 0;

        while (size >= 1024 && unitIndex < units.length - 1) {
            size /= 1024;
            unitIndex += 1;
        }

        const rounded = size >= 10 || unitIndex === 0 ? Math.round(size) : Number(size.toFixed(1));
        return `${rounded} ${units[unitIndex]}`;
    }

    function getDirectoryLabel(path: string): string {
        const normalized = path.replace(/\\/g, "/");
        const lastSlash = normalized.lastIndexOf("/");
        return lastSlash === -1 ? path : normalized.slice(0, lastSlash);
    }

    function getRecentFileTypeToken(recentFile: RecentFileRecord): string {
        const normalizedExtension = recentFile.extension?.replace(/^\./, "").trim().toUpperCase();
        if (normalizedExtension) {
            return normalizedExtension;
        }

        const detectedLanguage = getRecentFileLanguage(recentFile);
        const token = languageTypeTokens[detectedLanguage];
        if (token) {
            return token;
        }

        return languageMetaByValue.get(detectedLanguage)?.label.toUpperCase() ?? "FILE";
    }

    function getRecentFileLanguage(recentFile: RecentFileRecord): string {
        return languageDetector.detect("", recentFile.file_name)
            ?? languageDetector.detect("", recentFile.path)
            ?? "text";
    }

    function getRecentFileIcon(recentFile: RecentFileRecord): LanguageIcon | null {
        return languageMetaByValue.get(getRecentFileLanguage(recentFile))?.icon
            ?? languageMetaByValue.get("text")?.icon
            ?? null;
    }

    function getRecencyTimestamp(recentFile: RecentFileRecord): number | null {
        return recentFile.last_modified_at
            ?? recentFile.last_saved_at
            ?? recentFile.last_opened_at
            ?? recentFile.last_seen_at;
    }

    function getRecencyBucket(timestamp: number | null): RecencyBucket {
        if (!timestamp) {
            return "older";
        }

        const startOfToday = new Date();
        startOfToday.setHours(0, 0, 0, 0);

        if (timestamp >= startOfToday.getTime()) {
            return "today";
        }

        const startOfThisWeek = startOfToday.getTime() - (6 * 24 * 60 * 60 * 1000);
        return timestamp >= startOfThisWeek ? "this-week" : "older";
    }

    function buildRecencySections(
        files: RecentFileRecord[],
        sortOrder: Extract<SortMode, "last-modified" | "oldest">,
    ): RecentFileSection[] {
        const sectionItems: Record<RecencyBucket, RecentFileRecord[]> = {
            today: [],
            "this-week": [],
            older: [],
        };

        for (const recentFile of files) {
            sectionItems[getRecencyBucket(getRecencyTimestamp(recentFile))].push(recentFile);
        }

        return recencySectionOrder[sortOrder]
            .map((bucket) => ({
                key: bucket,
                label: recencySectionLabels[bucket],
                items: sectionItems[bucket],
            }))
            .filter((section) => section.items.length > 0);
    }

    function compareNumbers(left: number | null, right: number | null): number {
        if (left === right) {
            return 0;
        }

        if (left === null) {
            return 1;
        }

        if (right === null) {
            return -1;
        }

        return left - right;
    }

    function compareText(left: string | null | undefined, right: string | null | undefined): number {
        if (left === right) {
            return 0;
        }

        if (!left) {
            return 1;
        }

        if (!right) {
            return -1;
        }

        return textCollator.compare(left, right);
    }

    function getSourceLabel(source: RecentFileSource): string {
        return source === "internal" ? "Slate" : "External";
    }

    function getPrimaryTimestamp(recentFile: RecentFileRecord): number | null {
        return recentFile.last_opened_at
            ?? recentFile.last_saved_at
            ?? recentFile.last_modified_at
            ?? recentFile.last_seen_at;
    }

    function compareRecentFiles(
        left: RecentFileRecord,
        right: RecentFileRecord,
        sortOrder: SortMode,
    ): number {
        switch (sortOrder) {
            case "last-modified": {
                const byTimestamp = compareNumbers(
                    getRecencyTimestamp(right),
                    getRecencyTimestamp(left),
                );
                if (byTimestamp !== 0) {
                    return byTimestamp;
                }
                break;
            }
            case "oldest": {
                const byTimestamp = compareNumbers(
                    getRecencyTimestamp(left),
                    getRecencyTimestamp(right),
                );
                if (byTimestamp !== 0) {
                    return byTimestamp;
                }
                break;
            }
            case "name-asc": {
                const byName = compareText(left.file_name, right.file_name);
                if (byName !== 0) {
                    return byName;
                }
                break;
            }
            case "name-desc": {
                const byName = compareText(right.file_name, left.file_name);
                if (byName !== 0) {
                    return byName;
                }
                break;
            }
            case "size-desc": {
                const bySize = compareNumbers(right.size_bytes, left.size_bytes);
                if (bySize !== 0) {
                    return bySize;
                }
                break;
            }
            case "size-asc": {
                const bySize = compareNumbers(left.size_bytes, right.size_bytes);
                if (bySize !== 0) {
                    return bySize;
                }
                break;
            }
        }

        const byName = compareText(left.file_name, right.file_name);
        if (byName !== 0) {
            return byName;
        }

        return compareText(left.path, right.path);
    }

    async function refreshRecentFiles(): Promise<void> {
        const currentVersion = ++requestVersion;
        isLoading = true;
        loadError = "";

        try {
            const result = await getRecentFiles(RECENT_FILES_LIMIT);
            if (currentVersion !== requestVersion) {
                return;
            }

            recentFiles = result;
        } catch (error: unknown) {
            if (currentVersion !== requestVersion) {
                return;
            }

            loadError = typeof error === "string"
                ? error
                : "Failed to load recent files.";
        } finally {
            if (currentVersion === requestVersion) {
                isLoading = false;
            }
        }
    }

    async function openRecentFile(path: string): Promise<void> {
        const { emit } = await import("@tauri-apps/api/event");
        await emit(OPEN_FILE_PATH_EVENT, {
            path,
        } satisfies OpenFilePathPayload);
    }

    $effect(() => {
        void refreshRecentFiles();
    });

    $effect(() => {
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
        let disposed = false;
        let unlistenRecentFiles: undefined | (() => void);

        const setup = import("@tauri-apps/api/event").then(async ({ listen }) => {
            unlistenRecentFiles = await listen(RECENT_FILES_UPDATED_EVENT, () => {
                if (!disposed) {
                    void refreshRecentFiles();
                }
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
                    void refreshRecentFiles();
                }}
            >
                <RefreshCcw class={isLoading ? "size-4 animate-spin" : "size-4"} />
            </Button>
        </div>

        <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2 px-1">
            <div class="relative min-w-0">
                <Search class="pointer-events-none absolute left-4 top-1/2 size-4 -translate-y-1/2 text-sidebar-foreground/50" />
                <Input
                    bind:ref={searchInput}
                    bind:value={query}
                    placeholder="Search library..."
                    class="border-sidebar-border bg-sidebar ps-9 text-sm shadow-none placeholder:text-sidebar-foreground/45 focus-visible:border-sidebar-ring focus-visible:ring-sidebar-ring/40"
                />
            </div>

            <Select.Root type="single" items={sortOptions} bind:value={sortMode}>
                <Select.Trigger
                    aria-label={`Sort library: ${activeSortOption.label}`}
                    title={`Sort library: ${activeSortOption.label}`}
                    class="h-9 w-9 justify-center gap-0 border-sidebar-border bg-sidebar px-0 text-sidebar-foreground shadow-none focus-visible:border-sidebar-ring focus-visible:ring-sidebar-ring/40 [&>svg:last-child]:hidden"
                >
                    {@const ActiveSortIcon = activeSortOption.icon}
                    <span class="flex items-center justify-center">
                        <ActiveSortIcon class="size-4" />
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

    <div class="flex-1 min-h-0 overflow-auto p-2">
        <Sidebar.Group class="gap-2 p-0">
            {#if loadError}
                <div class="rounded-lg border border-destructive/30 bg-destructive/8 px-3 py-2 text-sm text-destructive">
                    {loadError}
                </div>
            {:else if isLoading && recentFiles.length === 0}
                <div class="space-y-2 px-1 pt-1">
                    {#each Array.from({ length: 5 }) as _, index (index)}
                        <div class="rounded-lg border border-sidebar-border/60 bg-sidebar-accent/40 px-3 py-3 animate-pulse">
                            <div class="h-3 w-3/5 rounded bg-sidebar-foreground/10"></div>
                            <div class="mt-2 h-2.5 w-4/5 rounded bg-sidebar-foreground/10"></div>
                        </div>
                    {/each}
                </div>
            {:else if visibleRecentFiles.length === 0}
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
                                    <span class="truncate whitespace-nowrap text-[11px] uppercase tracking-[0.12em] text-sidebar-foreground/50">
                                        {section.label}
                                    </span>
                                    <div class="h-px flex-1 bg-sidebar-border/55"></div>
                                    <span class="shrink-0 whitespace-nowrap text-[10px] text-sidebar-foreground/40">
                                        {section.items.length}
                                    </span>
                                </div>
                            {/if}

                            <Item.Group class="gap-2">
                                {#each section.items as recentFile (recentFile.path)}
                                    {@const FileIcon = getRecentFileIcon(recentFile)}
                                    {@const fileSize = formatSize(recentFile.size_bytes)}
                                    <Item.Root
                                        variant="outline"
                                        size="sm"
                                        class="overflow-hidden border-sidebar-border/65 bg-sidebar/35 p-0 shadow-none"
                                    >
                                        <button
                                            type="button"
                                            class="group flex w-full min-w-0 items-start gap-3 overflow-hidden px-3.5 py-3 text-left outline-none transition-colors hover:bg-sidebar-accent/70 hover:text-sidebar-accent-foreground focus-visible:bg-sidebar-accent/70 focus-visible:text-sidebar-accent-foreground"
                                            title={recentFile.path}
                                            onclick={() => {
                                                void openRecentFile(recentFile.path);
                                            }}
                                        >
                                            <Item.Media
                                                variant="icon"
                                                class="mt-0.5 border-sidebar-border/70 bg-sidebar-accent/45 text-sidebar-foreground/80 group-hover:border-sidebar-background/60 group-hover:bg-sidebar/80 group-hover:text-sidebar-accent-foreground"
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
                                                        <Item.Title class="truncate text-[15px] leading-tight text-sidebar-foreground group-hover:text-sidebar-accent-foreground">
                                                            {recentFile.file_name}
                                                        </Item.Title>

                                                        <Item.Description class="mt-1 truncate text-[11.5px] text-sidebar-foreground/62 group-hover:text-sidebar-accent-foreground/74">
                                                            {getDirectoryLabel(recentFile.path)}
                                                        </Item.Description>
                                                    </div>

                                                    {#if !recentFile.exists_on_disk}
                                                        <Item.Actions class="pt-0.5">
                                                            <span class="inline-flex shrink-0 items-center gap-1 whitespace-nowrap rounded-full border border-amber-500/25 bg-amber-500/10 px-2 py-1 text-[10px] font-medium uppercase tracking-[0.12em] text-amber-600 dark:text-amber-300">
                                                                <FileWarning class="size-3.5" />
                                                                Missing
                                                            </span>
                                                        </Item.Actions>
                                                    {/if}
                                                </div>

                                                <div class="flex min-w-0 flex-nowrap items-center gap-2 overflow-hidden text-[11px] text-sidebar-foreground/55 group-hover:text-sidebar-accent-foreground/72">
                                                    <span class="truncate whitespace-nowrap font-medium uppercase tracking-[0.12em] text-sidebar-foreground/72 group-hover:text-sidebar-accent-foreground/88">
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
                                    </Item.Root>
                                {/each}
                            </Item.Group>
                        </section>
                    {/each}
                </Sidebar.GroupContent>
            {/if}
        </Sidebar.Group>
    </div>
</div>
