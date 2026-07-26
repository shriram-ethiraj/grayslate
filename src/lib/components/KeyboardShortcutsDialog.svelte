<script lang="ts">
    import { onDestroy, tick } from "svelte";
    import * as Dialog from "$lib/components/ui/dialog/index.js";
    import { closeAppDialog, appDialogsState } from "$lib/state/appDialogs.svelte";
    import { platformState } from "$lib/state/platform.svelte";
    import SearchIcon from "~icons/lucide/search";
    import {
        formatShortcutKey,
        shortcutCategories,
        type ShortcutDefinition,
        type ShortcutKey,
    } from "$lib/shortcuts";

    const isOpen = $derived(appDialogsState.active.type === "keyboard-shortcuts");

    let query = $state("");
    let inputRef = $state<HTMLInputElement | null>(null);
    const normalizedQuery = $derived(query.trim().toLocaleLowerCase());

    type FilteredShortcutCategory = {
        id: string;
        label: string;
        shortcuts: readonly ShortcutDefinition[];
    };

    function formatShortcut(key: ShortcutKey): string {
        return formatShortcutKey(key, platformState.osType);
    }

    function getShortcutKeywords(
        categoryLabel: string,
        shortcut: ShortcutDefinition,
    ): string[] {
        return [
            categoryLabel,
            shortcut.label,
            ...shortcut.keys,
            ...shortcut.keys.map(formatShortcut),
        ];
    }

    const filteredCategories = $derived.by((): FilteredShortcutCategory[] => {
        return shortcutCategories
            .map((category) => ({
                id: category.id,
                label: category.label,
                shortcuts: category.shortcuts.filter((shortcut) => {
                    if (!normalizedQuery) return true;
                    return getShortcutKeywords(category.label, shortcut).some((keyword) =>
                        keyword.toLocaleLowerCase().includes(normalizedQuery),
                    );
                }),
            }))
            .filter((category) => category.shortcuts.length > 0);
    });

    async function focusSearch(): Promise<void> {
        await tick();
        inputRef?.focus();
    }

    onDestroy(() => {
        inputRef = null;
    });
</script>

<Dialog.Root
    open={isOpen}
    onOpenChange={(open) => {
        if (!open) closeAppDialog();
    }}
>
    <Dialog.Content
        data-testid="keyboard-shortcuts-dialog"
        class="gap-0 p-0 sm:max-w-[46rem]"
        onOpenAutoFocus={(event) => {
            event.preventDefault();
            query = "";
            void focusSearch();
        }}
    >
        <Dialog.Header class="sr-only">
            <Dialog.Title>Keyboard Shortcuts</Dialog.Title>
            <Dialog.Description>
                Search all of Grayslate's keyboard shortcuts.
            </Dialog.Description>
        </Dialog.Header>

        <!-- Keep clipping on an inner wrapper so the dialog's rounded ring stays
             stable on WebKitGTK while the shortcut list scrolls. -->
        <div
            class="m-px flex h-[34rem] max-h-[calc(100vh-4rem)] min-h-0 flex-col overflow-hidden rounded-md"
        >
            <div class="flex min-h-0 flex-1 flex-col bg-popover text-popover-foreground">
                <div
                    class="flex h-9 shrink-0 items-center gap-2 border-b ps-3 pe-8"
                    role="search"
                >
                    <SearchIcon class="size-4 shrink-0 opacity-50" />
                    <input
                        bind:this={inputRef}
                        bind:value={query}
                        data-testid="keyboard-shortcuts-search"
                        placeholder="Search keyboard shortcuts..."
                        aria-label="Search keyboard shortcuts"
                        autocomplete="off"
                        class="placeholder:text-muted-foreground h-10 w-full bg-transparent py-3 text-sm outline-none"
                    />
                </div>

                <div
                    data-testid="keyboard-shortcuts-list"
                    class="min-h-0 flex-1 overflow-x-hidden overflow-y-auto overscroll-none p-2"
                >
                    {#if filteredCategories.length === 0}
                        <div class="py-6 text-center text-sm">No shortcuts found.</div>
                    {/if}

                    {#each filteredCategories as category (category.id)}
                        <section class="p-1 pb-2" aria-labelledby={`shortcut-group-${category.id}`}>
                            <h3
                                id={`shortcut-group-${category.id}`}
                                class="px-2 py-1.5 text-xs font-medium text-muted-foreground"
                            >
                                {category.label}
                            </h3>
                            <ul>
                                {#each category.shortcuts as shortcut (shortcut.id)}
                                    <li
                                        data-testid={`shortcut-row-${shortcut.id}`}
                                        class="grid grid-cols-[minmax(0,1fr)_auto] gap-4 rounded-sm px-2 py-2.5"
                                    >
                                        <span class="min-w-0 text-sm text-foreground">
                                            {shortcut.label}
                                        </span>
                                        <span
                                            class="flex flex-wrap justify-end gap-1"
                                            aria-label={shortcut.keys
                                                .map(formatShortcut)
                                                .join(" or ")}
                                        >
                                            {#each shortcut.keys as key, keyIndex (key)}
                                                {#if keyIndex > 0}
                                                    <span
                                                        class="px-0.5 text-xs text-muted-foreground"
                                                        aria-hidden="true"
                                                    >
                                                        or
                                                    </span>
                                                {/if}
                                                <kbd
                                                    class="whitespace-nowrap rounded border bg-muted px-1.5 py-0.5 font-mono text-xs text-muted-foreground shadow-sm"
                                                >
                                                    {formatShortcut(key)}
                                                </kbd>
                                            {/each}
                                        </span>
                                    </li>
                                {/each}
                            </ul>
                        </section>
                    {/each}
                </div>
            </div>
        </div>
    </Dialog.Content>
</Dialog.Root>
