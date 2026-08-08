<script lang="ts">
    import * as Dialog from "$lib/components/ui/dialog/index.js";
    import * as Select from "$lib/components/ui/select/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import { Badge } from "$lib/components/ui/badge/index.js";
    import { Switch } from "$lib/components/ui/switch/index.js";
    import { Separator } from "$lib/components/ui/separator/index.js";
    import type { Component } from "svelte";
    import SettingsIcon from "~icons/lucide/settings";
    import FileCodeIcon from "~icons/lucide/file-code";
    import { appDialogsState, closeAppDialog } from "$lib/state/appDialogs.svelte";
    import {
        appSettingsState,
        setAutomaticUpdateChecks,
        setConfirmBeforeDelete,
        setDefaultIndentMode,
        setDefaultIndentSize,
        setDefaultEncoding,
        setDefaultLineEnding,
        setStartupBehavior,
        CHARACTER_ENCODING_OPTIONS,
        type DefaultCharacterEncoding,
        type DefaultIndentMode,
        type DefaultLineEnding,
        type StartupBehavior,
    } from "$lib/state/appSettings.svelte";
    import { appMenuState } from "$lib/state/appMenu.svelte";

    const isOpen = $derived(appDialogsState.active.type === "settings");

    type PaneId = "general" | "editor";
    const panes: { id: PaneId; label: string; icon: Component }[] = [
        { id: "general", label: "General", icon: SettingsIcon },
        { id: "editor", label: "Editor", icon: FileCodeIcon },
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
    // Applies to new documents only — an existing file always keeps whatever
    // line ending was detected when it was opened.
    const lineEndingOptions: { value: DefaultLineEnding; label: string }[] = [
        { value: "lf", label: "LF (Unix, macOS, Linux)" },
        { value: "crlf", label: "CRLF (Windows)" },
    ];

    const startupLabel = $derived(
        startupOptions.find((o) => o.value === appSettingsState.startupBehavior)?.label ?? "",
    );
    const indentModeLabel = $derived(
        indentModeOptions.find((o) => o.value === appSettingsState.defaultIndentMode)?.label ?? "",
    );
    const indentSizeLabel = $derived(
        appSettingsState.defaultIndentMode === "tab" ? "Tab Width" : "Indent Size",
    );
    const lineEndingLabel = $derived(
        lineEndingOptions.find((o) => o.value === appSettingsState.defaultLineEnding)?.label ?? "",
    );
    const encodingLabel = $derived(
        CHARACTER_ENCODING_OPTIONS.find((o) => o.value === appSettingsState.defaultEncoding)?.label ??
            "UTF-8",
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
                class="flex flex-col gap-0.5 border-r bg-state-track/60 p-2"
                aria-label="Settings sections"
            >
                <p class="px-2 pb-1.5 pt-1 text-xs font-semibold text-muted-foreground">
                    Settings
                </p>
                {#each panes as pane (pane.id)}
                    <Button
                        variant="ghost"
                        size="sm"
                        data-testid={`settings-pane-${pane.id}`}
                        class="h-8 w-full justify-start px-2 aria-pressed:font-medium"
                        aria-pressed={activePane === pane.id}
                        onclick={() => (activePane = pane.id)}
                    >
                        <pane.icon class="size-4 shrink-0" />
                        {pane.label}
                    </Button>
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
                                    class="text-sm font-normal text-foreground"
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
                                <Select.Trigger data-testid="settings-startup" class="w-full" id="settings-startup">
                                    {startupLabel}
                                </Select.Trigger>
                                <Select.Content>
                                    {#each startupOptions as option (option.value)}
                                        <Select.Item
                                            value={option.value}
                                            label={option.label}
                                            data-testid="settings-startup-{option.value}"
                                        >
                                            {option.label}
                                        </Select.Item>
                                    {/each}
                                </Select.Content>
                            </Select.Root>
                        </div>

                        <Separator />

                        <!-- Confirm before delete -->
                        <div class="flex items-center justify-between gap-4 py-4">
                            <div class="grid gap-0.5">
                                <span class="text-sm font-normal text-foreground">
                                    Confirm before deleting
                                </span>
                                <p class="text-xs text-muted-foreground">
                                    Show a confirmation dialog before permanently deleting a file.
                                </p>
                            </div>
                            <Switch
                                data-testid="settings-confirm-delete"
                                checked={appSettingsState.confirmBeforeDelete}
                                onCheckedChange={(checked) => setConfirmBeforeDelete(checked)}
                                aria-label="Confirm before deleting"
                            />
                        </div>

                        {#if appMenuState.updatePolicy === "self-update"}
                            <Separator />

                            <div class="flex items-center justify-between gap-4 py-4">
                                <div class="grid gap-0.5">
                                    <span class="text-sm font-normal text-foreground">
                                        Automatically check for updates
                                    </span>
                                    <p class="text-xs text-muted-foreground">
                                        Check at startup and once a day. Updates are never downloaded
                                        or installed automatically.
                                    </p>
                                </div>
                                <Switch
                                    data-testid="settings-automatic-update-checks"
                                    checked={appSettingsState.automaticUpdateChecks}
                                    onCheckedChange={(checked) => setAutomaticUpdateChecks(checked)}
                                    aria-label="Automatically check for updates"
                                />
                            </div>
                        {/if}
                    </div>
                {:else if activePane === "editor"}
                    <h2 class="mb-2 text-base font-semibold text-foreground">Editor</h2>
                    <div class="flex flex-col">
                        <!-- Default indentation -->
                        <div class="grid gap-2 py-4">
                            <div class="grid gap-0.5">
                                <span class="text-sm font-normal text-foreground">
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
                                    <Select.Trigger
                                        data-testid="settings-indent-mode"
                                        class="w-full"
                                        aria-label="Indent mode"
                                    >
                                        {indentModeLabel}
                                    </Select.Trigger>
                                    <Select.Content>
                                        {#each indentModeOptions as option (option.value)}
                                            <Select.Item
                                                value={option.value}
                                                label={option.label}
                                                data-testid="settings-indent-mode-{option.value}"
                                            >
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
                                    <Select.Trigger
                                        data-testid="settings-indent-size"
                                        class="w-full"
                                        aria-label={indentSizeLabel}
                                    >
                                        {appSettingsState.defaultIndentSize}
                                    </Select.Trigger>
                                    <Select.Content>
                                        {#each indentSizeOptions as option (option.value)}
                                            <Select.Item
                                                value={option.value}
                                                label={option.label}
                                                data-testid="settings-indent-size-{option.value}"
                                            >
                                                {option.label}
                                            </Select.Item>
                                        {/each}
                                    </Select.Content>
                                </Select.Root>
                            </div>
                        </div>

                        <Separator />

                        <!-- Default line ending -->
                        <div class="grid gap-2 py-4">
                            <div class="grid gap-0.5">
                                <label
                                    class="text-sm font-normal text-foreground"
                                    for="settings-line-ending"
                                >
                                    Default line ending
                                </label>
                                <p class="text-xs text-muted-foreground">
                                    Used for new slates. Existing files keep the line ending they
                                    were opened with.
                                </p>
                            </div>
                            <Select.Root
                                type="single"
                                value={appSettingsState.defaultLineEnding}
                                onValueChange={(v) =>
                                    setDefaultLineEnding(v as DefaultLineEnding)}
                            >
                                <Select.Trigger
                                    data-testid="settings-line-ending"
                                    class="w-full"
                                    id="settings-line-ending"
                                >
                                    {lineEndingLabel}
                                </Select.Trigger>
                                <Select.Content>
                                    {#each lineEndingOptions as option (option.value)}
                                        <Select.Item
                                            value={option.value}
                                            label={option.label}
                                            data-testid="settings-line-ending-{option.value}"
                                        >
                                            <span class="flex items-center gap-2">
                                                {option.label}
                                                {#if option.value === "lf"}
                                                    <Badge variant="secondary" class="px-1.5 py-0 text-xs">
                                                        Recommended
                                                    </Badge>
                                                {/if}
                                            </span>
                                        </Select.Item>
                                    {/each}
                                </Select.Content>
                            </Select.Root>
                        </div>

                        <Separator />

                        <div class="grid gap-2 py-4">
                            <div class="grid gap-0.5">
                                <label
                                    class="text-sm font-normal text-foreground"
                                    for="settings-character-encoding"
                                >
                                    Default character encoding
                                </label>
                                <p class="text-xs text-muted-foreground">
                                    Used for new slates. Existing files keep the encoding they were
                                    opened with.
                                </p>
                            </div>
                            <Select.Root
                                type="single"
                                value={appSettingsState.defaultEncoding}
                                onValueChange={(v) =>
                                    setDefaultEncoding(v as DefaultCharacterEncoding)}
                            >
                                <Select.Trigger
                                    data-testid="settings-character-encoding"
                                    class="w-full"
                                    id="settings-character-encoding"
                                >
                                    {encodingLabel}
                                </Select.Trigger>
                                <Select.Content>
                                    {#each CHARACTER_ENCODING_OPTIONS as option (option.value)}
                                        <Select.Item
                                            value={option.value}
                                            label={option.label}
                                            data-testid="settings-character-encoding-{option.value}"
                                        >
                                            <span class="flex items-center gap-2">
                                                {option.label}
                                                {#if option.value === "utf-8"}
                                                    <Badge variant="secondary" class="px-1.5 py-0 text-xs">
                                                        Recommended
                                                    </Badge>
                                                {/if}
                                            </span>
                                        </Select.Item>
                                    {/each}
                                </Select.Content>
                            </Select.Root>
                        </div>
                    </div>
                {/if}
            </div>
        </div>
    </Dialog.Content>
</Dialog.Root>
