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
    import { languages } from "$lib/utils/languages";

    let selectedLabel = $derived.by(() => {
        if (language === "auto") {
            const detectedLang = languages.find((l) => l.value === detectedLanguage);
            const detectedLabel = detectedLang?.label ?? "Plain text";
            return { label: `Auto (${detectedLabel})`, icon: detectedLang?.icon };
        }
        const lang = languages.find((l) => l.value === language);
        return { label: lang?.label ?? "Plain text", icon: lang?.icon };
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
                {#if selectedLabel.icon}
                    {#if "svg" in selectedLabel.icon}
                        <div class="w-3 h-3 flex items-center justify-center" style="fill: currentColor;">
                            {@html selectedLabel.icon.svg}
                        </div>
                    {:else}
                        {@const Icon = selectedLabel.icon}
                        <Icon class="w-3 h-3" />
                    {/if}
                {/if}
                {selectedLabel.label}
            </Select.Trigger>
            <Select.Content class="text-[11px]">
                {#each languages as lang}
                    <Select.Item class="text-[11px] flex items-center gap-2" value={lang.value}>
                        {#if lang.icon}
                            {#if "svg" in lang.icon}
                                <div class="w-3 h-3 flex items-center justify-center" style="fill: currentColor;">
                                    {@html lang.icon.svg}
                                </div>
                            {:else}
                                {@const Icon = lang.icon}
                                <Icon class="w-3 h-3" />
                            {/if}
                        {:else}
                            <div class="w-3 h-3"></div>
                        {/if}
                        {lang.label}
                    </Select.Item>
                {/each}
            </Select.Content>
        </Select.Root>
    </div>
</div>
