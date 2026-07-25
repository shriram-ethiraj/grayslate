<script module lang="ts">
    import type { Eol } from "$lib/state/appSettings.svelte";

    /**
     * Status-bar labels. These are the names the editor world uses — VS Code,
     * Sublime, and Notepad++ all surface "LF"/"CRLF" rather than "Unix"/
     * "Windows" in the status bar itself.
     */
    export const EOL_LABELS: Record<Eol, string> = {
        lf: "LF",
        crlf: "CRLF",
    };
</script>

<script lang="ts">
    import * as Dialog from "$lib/components/ui/dialog/index.js";
    import * as Select from "$lib/components/ui/select/index.js";
    import {
        openEolPicker,
        registerEditorPopup,
        syncEditorPopupOpenState,
    } from "$lib/state/editor.svelte";
    import { AppTooltip } from "$lib/components/ui/tooltip/index.js";

    let {
        eol = "lf",
        onChange = () => {},
    }: {
        eol: Eol;
        onChange?: (next: Eol) => void;
    } = $props();

    let open = $state(false);

    const options: { value: Eol; label: string }[] = [
        { value: "lf", label: "LF (Unix, macOS, Linux)" },
        { value: "crlf", label: "CRLF (Windows)" },
    ];

    function handleValueChange(next: string) {
        if (next !== "lf" && next !== "crlf") return;
        onChange(next);
    }

    const activeLabel = $derived(
        options.find((option) => option.value === eol)?.label ?? EOL_LABELS[eol],
    );

    $effect(() => {
        syncEditorPopupOpenState("eol-picker", open);
    });

    $effect(() => {
        return registerEditorPopup("eol-picker", {
            open: (request) => {
                if (request.id !== "eol-picker") return;
                open = true;
            },
            close: () => {
                open = false;
            },
        });
    });
</script>

<!-- Status bar trigger button -->
<AppTooltip content="Select end of line sequence">
    {#snippet trigger({ props })}
        <button
            {...props}
            type="button"
            onclick={openEolPicker}
            class="ui-state flex h-full cursor-pointer items-center rounded-none px-2 text-xs"
            data-testid="status-eol"
            data-eol={eol}
        >
            {EOL_LABELS[eol]}
        </button>
    {/snippet}
</AppTooltip>

<Dialog.Root bind:open>
    <Dialog.Content
        data-testid="eol-picker-dialog"
        class="sm:max-w-88"
        showCloseButton={true}
    >
        <Dialog.Header class="gap-0.5 pr-8 text-left">
            <Dialog.Title class="text-sm font-normal leading-normal">
                Line ending
            </Dialog.Title>
            <Dialog.Description class="text-xs leading-normal">
                Choose how new lines are saved in this file.
            </Dialog.Description>
        </Dialog.Header>

        <Select.Root type="single" value={eol} onValueChange={handleValueChange}>
            <Select.Trigger
                data-testid="eol-select-trigger"
                class="w-full"
                aria-label="Line ending"
            >
                {activeLabel}
            </Select.Trigger>
            <Select.Content>
                {#each options as option (option.value)}
                    <Select.Item
                        value={option.value}
                        label={option.label}
                        data-testid={`eol-item-${option.value}`}
                    >
                        {option.label}
                    </Select.Item>
                {/each}
            </Select.Content>
        </Select.Root>
    </Dialog.Content>
</Dialog.Root>
