<script lang="ts">
    import { onDestroy, tick } from "svelte";
    import { formatForDisplay } from "@tanstack/hotkeys";
    import * as Command from "$lib/components/ui/command/index.js";
    import * as Dialog from "$lib/components/ui/dialog/index.js";
    import { closeAppDialog, appDialogsState } from "$lib/state/appDialogs.svelte";
    import { platformState } from "$lib/state/platform.svelte";
    import {
        getShortcutPlatform,
        shortcutCategories,
        type ShortcutDefinition,
        type ShortcutKey,
    } from "$lib/shortcuts";

    const isOpen = $derived(appDialogsState.active.type === "keyboard-shortcuts");
    const displayPlatform = $derived(getShortcutPlatform(platformState.osType));

    let query = $state("");
    let inputRef = $state<HTMLInputElement | null>(null);

    function formatShortcut(key: ShortcutKey): string {
        return formatForDisplay(key, { platform: displayPlatform });
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
            <Command.Root class="min-h-0">
                <Command.Input
                    bind:ref={inputRef}
                    bind:value={query}
                    data-testid="keyboard-shortcuts-search"
                    placeholder="Search keyboard shortcuts..."
                    aria-label="Search keyboard shortcuts"
                />

                <Command.List class="min-h-0 max-h-none flex-1 p-2">
                    <Command.Empty>No shortcuts found.</Command.Empty>

                    {#each shortcutCategories as category (category.id)}
                        <Command.Group heading={category.label} class="p-1 pb-2">
                            {#each category.shortcuts as shortcut (shortcut.id)}
                                <Command.Item
                                    value={`${category.label}: ${shortcut.label}`}
                                    keywords={getShortcutKeywords(category.label, shortcut)}
                                    data-testid={`shortcut-row-${shortcut.id}`}
                                    class="grid cursor-default grid-cols-[minmax(0,1fr)_auto] gap-4 px-2 py-2.5"
                                >
                                    <span class="min-w-0 text-sm text-foreground">
                                        {shortcut.label}
                                    </span>
                                    <span
                                        class="flex flex-wrap justify-end gap-1"
                                        aria-label={shortcut.keys.map(formatShortcut).join(" or ")}
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
                                </Command.Item>
                            {/each}
                        </Command.Group>
                    {/each}
                </Command.List>
            </Command.Root>
        </div>
    </Dialog.Content>
</Dialog.Root>
