<script lang="ts">
    import SquareSplitHorizontal from "~icons/lucide/square-split-horizontal";
    import Table2 from "~icons/lucide/table-2";
    import LanguagePicker from "./LanguagePicker.svelte";

    let {
        line,
        col,
        selectionSize = 0,
        language = $bindable("auto"),
        detectedLanguage = "text",
        activeLanguage = "text",
        isCsvTableActive = false,
        csvInfo = { rows: 0, cols: 0, delimiter: "", errors: 0 },
    } = $props();
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
                Ln {line}, Col {col}{#if selectionSize > 0}&nbsp;({selectionSize}
                    selected){/if}
            </button>
        {/if}
        <LanguagePicker bind:language {detectedLanguage} />
    </div>
</div>
