<script lang="ts">
    import { tick } from "svelte";
    import * as Sidebar from "$lib/components/ui/sidebar/index.js";
    import * as Select from "$lib/components/ui/select/index.js";
    import * as Tabs from "$lib/components/ui/tabs/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import Input from "$lib/components/ui/input/input.svelte";
    import type { FilterMode, SortMode } from "$lib/files/sidebarUtils";
    import Search from "~icons/lucide/search";
    import RefreshCcw from "~icons/lucide/refresh-ccw";
    import Files from "~icons/lucide/files";
    import Clock3 from "~icons/lucide/clock-3";
    import History from "~icons/lucide/history";
    import ArrowDownAZ from "~icons/lucide/arrow-down-a-z";
    import ArrowUpZA from "~icons/lucide/arrow-up-z-a";
    import ArrowDownWideNarrow from "~icons/lucide/arrow-down-wide-narrow";
    import ArrowUpNarrowWide from "~icons/lucide/arrow-up-narrow-wide";
    import RiCodeBoxLine from "~icons/ri/code-box-line";
    import LucideHardDrive from "~icons/lucide/hard-drive";

    interface Props {
        query: string;
        filterMode: FilterMode;
        sortMode: SortMode;
        isLoading: boolean;
        isSearchLoading: boolean;
        /**
         * Incrementing counter — header focuses the search input whenever this
         * bumps. The parent increments it; the header owns the DOM ref and effect.
         */
        focusRequest: number;
        onRefresh: () => void;
    }

    let {
        query = $bindable(),
        filterMode = $bindable(),
        sortMode = $bindable(),
        isLoading,
        isSearchLoading,
        focusRequest,
        onRefresh,
    }: Props = $props();

    // Owned by this component; never needs to leave.
    let searchInput = $state<HTMLInputElement | null>(null);

    // Used to guard focus when the sidebar panel is collapsed.
    const sidebar = Sidebar.useSidebar();

    // ---------------------------------------------------------------------------
    // Static option lists (live here because they reference icon components)
    // ---------------------------------------------------------------------------

    const filterOptions: Array<{
        value: FilterMode;
        label: string;
        title: string;
        icon: typeof Files;
    }> = [
        { value: "unified", label: "All", title: "Show all recently opened files", icon: Files },
        { value: "slates", label: "Slates", title: "Show Grayslate documents only", icon: RiCodeBoxLine },
        { value: "local", label: "Local", title: "Show previously opened local files only", icon: LucideHardDrive },
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

    const activeSortOption = $derived(sortOptions.find((o) => o.value === sortMode) ?? sortOptions[0]);

    // ---------------------------------------------------------------------------
    // Focus management
    // ---------------------------------------------------------------------------

    $effect(() => {
        // Reading focusRequest subscribes to its changes; each bump triggers a focus.
        focusRequest;

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
</script>

<Sidebar.Group class="shrink-0 gap-2 border-b border-sidebar-border/70 px-2 py-2">
    <div class="flex items-center justify-between gap-2 px-1">
        <div class="min-w-0 truncate text-sm font-medium">Library</div>
        <Button
            variant="ghost"
            size="icon-sm"
            class="text-sidebar-foreground/70 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
            aria-label="Refresh recent files"
            title="Refresh recent files"
            onclick={onRefresh}
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
