<script lang="ts">
    import * as Dialog from "$lib/components/ui/dialog/index.js";
    import * as Select from "$lib/components/ui/select/index.js";
    import { Switch } from "$lib/components/ui/switch/index.js";
    import { Separator } from "$lib/components/ui/separator/index.js";
    import type { Component } from "svelte";
    import SettingsIcon from "~icons/lucide/settings";
    import { appDialogsState, closeAppDialog } from "$lib/state/appDialogs.svelte";
    import {
        appSettingsState,
        setConfirmBeforeDelete,
        setDefaultIndentMode,
        setDefaultIndentSize,
        setStartupBehavior,
        type DefaultIndentMode,
        type StartupBehavior,
    } from "$lib/state/appSettings.svelte";

    const isOpen = $derived(appDialogsState.active.type === "settings");

    // Settings panes. Only "general" ships today; Git sync and Themes are
    // planned future panes — adding one is a matter of appending to this list
    // and rendering another branch in the content area.
    type PaneId = "general";
    const panes: { id: PaneId; label: string; icon: Component }[] = [
        { id: "general", label: "General", icon: SettingsIcon},
    ];
    let activePane = $state<PaneId>("general");

    const startupOptions: { value: StartupBehavior; label: string }[] = [
        { value: "new", label: "Start with a new slate" },
        { value: "last", label: "Reopen last file" },
    ];
    const indentModeOptions: { value: DefaultIndentMode; label: string }[] = [
        { value: "spaces", label: "Spaces" },
        { value: "tab", label: "Tab" },
    ];
    const indentSizeOptions = Array.from({ length: 8 }, (_, i) => ({
        value: String(i + 1),
        label: String(i + 1),
    }));

    const startupLabel = $derived(
        startupOptions.find((o) => o.value === appSettingsState.startupBehavior)?.label ?? "",
    );
    const indentModeLabel = $derived(
        indentModeOptions.find((o) => o.value === appSettingsState.defaultIndentMode)?.label ?? "",
    );
    const indentSizeLabel = $derived(
        appSettingsState.defaultIndentMode === "tab" ? "Tab Width" : "Indent Size",
    );
</script>

<Dialog.Root
    open={isOpen}
    onOpenChange={(open) => {
        if (!open) closeAppDialog();
    }}
>
    <Dialog.Content data-testid="settings-dialog" class="gap-0 overflow-hidden p-0 sm:max-w-[46rem]">
        <!-- Accessible name/description; the visible layout is custom. -->
        <Dialog.Header class="sr-only">
            <Dialog.Title>Settings</Dialog.Title>
            <Dialog.Description>Configure Grayslate preferences.</Dialog.Description>
        </Dialog.Header>

        <div class="grid h-[26rem] grid-cols-[11rem_1fr]">
            <!-- Left nav rail -->
            <nav
                class="flex flex-col gap-0.5 border-r bg-muted/30 p-2"
                aria-label="Settings sections"
            >
                <p class="px-2 pb-1.5 pt-1 text-xs font-semibold text-muted-foreground">
                    Settings
                </p>
                {#each panes as pane (pane.id)}
                    <button
                        type="button"
                        class="flex items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground data-[active=true]:bg-accent data-[active=true]:font-medium data-[active=true]:text-accent-foreground"
                        data-active={activePane === pane.id}
                        onclick={() => (activePane = pane.id)}
                    >
                        <pane.icon class="size-4 shrink-0" />
                        {pane.label}
                    </button>
                {/each}
            </nav>

            <!-- Content pane -->
            <div class="overflow-y-auto p-6">
                {#if activePane === "general"}
                    <h2 class="mb-2 text-base font-semibold text-foreground">General</h2>
                    <div class="flex flex-col">
                        <!-- Startup behavior -->
                        <div class="grid gap-2 py-4">
                            <div class="grid gap-0.5">
                                <label
                                    class="text-sm font-medium text-foreground"
                                    for="settings-startup"
                                >
                                    On startup
                                </label>
                                <p class="text-xs text-muted-foreground">
                                    Choose what Grayslate opens when it launches.
                                </p>
                            </div>
                            <Select.Root
                                type="single"
                                value={appSettingsState.startupBehavior}
                                onValueChange={(v) => setStartupBehavior(v as StartupBehavior)}
                            >
                                <Select.Trigger class="w-full" id="settings-startup">
                                    {startupLabel}
                                </Select.Trigger>
                                <Select.Content>
                                    {#each startupOptions as option (option.value)}
                                        <Select.Item value={option.value} label={option.label}>
                                            {option.label}
                                        </Select.Item>
                                    {/each}
                                </Select.Content>
                            </Select.Root>
                        </div>

                        <Separator />

                        <!-- Default indentation -->
                        <div class="grid gap-2 py-4">
                            <div class="grid gap-0.5">
                                <span class="text-sm font-medium text-foreground">
                                    Default indentation
                                </span>
                                <p class="text-xs text-muted-foreground">
                                    Used for new slates and files without their own indentation.
                                </p>
                            </div>
                            <div class="grid grid-cols-2 gap-3">
                                <Select.Root
                                    type="single"
                                    value={appSettingsState.defaultIndentMode}
                                    onValueChange={(v) =>
                                        setDefaultIndentMode(v as DefaultIndentMode)}
                                >
                                    <Select.Trigger class="w-full" aria-label="Indent mode">
                                        {indentModeLabel}
                                    </Select.Trigger>
                                    <Select.Content>
                                        {#each indentModeOptions as option (option.value)}
                                            <Select.Item value={option.value} label={option.label}>
                                                {option.label}
                                            </Select.Item>
                                        {/each}
                                    </Select.Content>
                                </Select.Root>
                                <Select.Root
                                    type="single"
                                    value={String(appSettingsState.defaultIndentSize)}
                                    onValueChange={(v) => setDefaultIndentSize(Number(v))}
                                >
                                    <Select.Trigger class="w-full" aria-label={indentSizeLabel}>
                                        {appSettingsState.defaultIndentSize}
                                    </Select.Trigger>
                                    <Select.Content>
                                        {#each indentSizeOptions as option (option.value)}
                                            <Select.Item value={option.value} label={option.label}>
                                                {option.label}
                                            </Select.Item>
                                        {/each}
                                    </Select.Content>
                                </Select.Root>
                            </div>
                        </div>

                        <Separator />

                        <!-- Confirm before delete -->
                        <div class="flex items-center justify-between gap-4 py-4">
                            <div class="grid gap-0.5">
                                <span class="text-sm font-medium text-foreground">
                                    Confirm before deleting
                                </span>
                                <p class="text-xs text-muted-foreground">
                                    Show a confirmation dialog before permanently deleting a file.
                                </p>
                            </div>
                            <Switch
                                checked={appSettingsState.confirmBeforeDelete}
                                onCheckedChange={(checked) => setConfirmBeforeDelete(checked)}
                                aria-label="Confirm before deleting"
                            />
                        </div>
                    </div>
                {/if}
            </div>
        </div>
    </Dialog.Content>
</Dialog.Root>
