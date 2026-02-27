<script lang="ts">
    import * as Select from "$lib/components/ui/select/index.js";

    let {
        line,
        col,
        language = $bindable("auto"),
        detectedLanguage = "text",
        activeLanguage = "text",
        isCsvTableActive = false,
        csvInfo = { rows: 0, cols: 0, delimiter: "", errors: 0 },
    } = $props();

    import { SquareSplitHorizontal, Table2 } from "@lucide/svelte";

    const languages = [
        { value: "auto", label: "Auto Detect" },
        { value: "text", label: "Plain text" },
        { value: "json", label: "JSON" },
        { value: "javascript", label: "JavaScript" },
        { value: "typescript", label: "TypeScript" },
        { value: "python", label: "Python" },
        { value: "html", label: "HTML" },
        { value: "css", label: "CSS" },
        { value: "yaml", label: "YAML" },
        { value: "c", label: "C" },
        { value: "cpp", label: "C++" },
        { value: "java", label: "Java" },
        { value: "go", label: "Go" },
        { value: "xml", label: "XML" },
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
        {#if isCsvTableActive}
            <div
                class="flex items-center gap-3 px-2 h-full cursor-default border-r border-border/40"
            >
                <span>{csvInfo.rows} rows × {csvInfo.cols} cols</span>
                <span
                    >Delimiter: <strong class="font-semibold"
                        >{csvInfo.delimiter}</strong
                    ></span
                >
                {#if csvInfo.errors > 0}
                    <span class="text-[hsl(0,80%,60%)]">
                        ⚠ {csvInfo.errors} parse error{csvInfo.errors > 1
                            ? "s"
                            : ""}
                    </span>
                {/if}
            </div>
        {:else}
            <button
                class="flex items-center hover:bg-muted/50 hover:text-foreground h-full px-2 transition-colors cursor-default"
            >
                Ln {line}, Col {col}
            </button>
        {/if}
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
