<script lang="ts">
    import * as Select from "$lib/components/ui/select/index.js";

    let {
        line,
        col,
        language = $bindable("auto"),
        detectedLanguage = "text",
        activeLanguage = "text",
        showPreview = $bindable(false),
        showCsvTable = $bindable(false),
    } = $props();

    import { SquareSplitHorizontal, Table2 } from "@lucide/svelte";

    const languages = [
        { value: "auto", label: "Auto Detect" },
        { value: "text", label: "Plain text" },
        { value: "json", label: "JSON" },
        { value: "javascript", label: "JavaScript" },
        { value: "python", label: "Python" },
        { value: "csv", label: "CSV" },
        { value: "markdown", label: "Markdown" },
    ];

    let selectedLabel = $derived.by(() => {
        if (language === "auto") {
            const detectedLabel =
                languages.find((l) => l.value === detectedLanguage)?.label ??
                "Plain text";
            return `Auto (${detectedLabel})`;
        }
        return (
            languages.find((l) => l.value === language)?.label ?? "Plain text"
        );
    });
</script>

<div
    class="flex h-6 w-full shrink-0 items-center justify-between px-3 text-[11px] bg-sidebar border-t border-border/40 text-muted-foreground select-none font-medium"
>
    <div class="flex items-center space-x-3 h-full">
        <!-- Left side could have errors, branch, etc. in future -->
    </div>
    <div class="flex items-center h-full">
        {#if activeLanguage === "csv"}
            <button
                class="flex items-center hover:bg-muted/50 {showCsvTable
                    ? 'text-primary'
                    : 'text-muted-foreground'} h-full px-2 transition-colors cursor-pointer gap-1.5 border-r border-border/40"
                onclick={() => (showCsvTable = !showCsvTable)}
                title="Toggle Table View"
            >
                <Table2 class="w-3.5 h-3.5" />
                Table
            </button>
        {/if}
        {#if activeLanguage === "markdown"}
            <button
                class="flex items-center hover:bg-muted/50 {showPreview
                    ? 'text-primary'
                    : 'text-muted-foreground'} h-full px-2 transition-colors cursor-pointer gap-1.5 border-r border-border/40"
                onclick={() => (showPreview = !showPreview)}
                title="Toggle Markdown Preview"
            >
                <SquareSplitHorizontal class="w-3.5 h-3.5" />
                Preview
            </button>
        {/if}
        <button
            class="flex items-center hover:bg-muted/50 hover:text-foreground h-full px-2 transition-colors cursor-default"
        >
            Ln {line}, Col {col}
        </button>
        <Select.Root type="single" bind:value={language}>
            <Select.Trigger
                class="flex items-center hover:bg-muted/50 hover:text-foreground h-full px-2 transition-colors cursor-default border-0 shadow-none focus:ring-0 rounded-none bg-transparent hocus:bg-muted/50 text-[11px] w-auto gap-2"
            >
                {selectedLabel}
            </Select.Trigger>
            <Select.Content class="text-[11px]">
                {#each languages as lang}
                    <Select.Item class="text-[11px]" value={lang.value}
                        >{lang.label}</Select.Item
                    >
                {/each}
            </Select.Content>
        </Select.Root>
    </div>
</div>
