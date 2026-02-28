<script lang="ts">
    import * as Command from "$lib/components/ui/command/index.js";
    import * as Dialog from "$lib/components/ui/dialog/index.js";
    import { languages } from "$lib/editor/config/supportedLanguages";
    import { Check } from "@lucide/svelte";

    let {
        language = $bindable("auto"),
        detectedLanguage = "text",
    }: {
        language: string;
        detectedLanguage: string;
    } = $props();

    let open = $state(false);

    const selectedLabel = $derived.by(() => {
        if (language === "auto") {
            const detectedLang = languages.find((l) => l.value === detectedLanguage);
            const detectedLabel = detectedLang?.label ?? "Plain text";
            return { label: `Auto (${detectedLabel})`, icon: detectedLang?.icon };
        }
        const lang = languages.find((l) => l.value === language);
        return { label: lang?.label ?? "Plain text", icon: lang?.icon };
    });

    function selectLanguage(value: string) {
        language = value;
        open = false;
    }

    // Detected language metadata for auto detect display
    const detectedLangMeta = $derived(
        languages.find((l) => l.value === detectedLanguage) ?? null
    );

    // Language list: all except auto
    const languageItems = languages.filter((l) => l.value !== "auto");
</script>

<!-- Status bar trigger button -->
<button
    onclick={() => (open = true)}
    class="flex items-center hover:bg-muted/50 hover:text-foreground h-full px-2 transition-colors cursor-pointer rounded-none bg-transparent text-[11px] gap-1.5"
    title="Select Language Mode"
>
    {#if selectedLabel.icon}
        {#if "svg" in selectedLabel.icon}
            <div class="w-3.5 h-3.5 flex items-center justify-center shrink-0 self-center [&_svg]:size-full [&_svg]:block" style="fill: currentColor;">
                {@html selectedLabel.icon.svg}
            </div>
        {:else}
            {@const Icon = selectedLabel.icon}
            <Icon class="w-3.5 h-3.5 shrink-0 self-center" strokeWidth={2.5} />
        {/if}
    {/if}
    {selectedLabel.label}
</button>

<!-- Language picker dialog -->
<Dialog.Root bind:open>
    <Dialog.Content
        class="overflow-hidden p-0 sm:max-w-[560px] gap-0"
        showCloseButton={false}
    >
        <span class="sr-only">Select Language Mode</span>
        <Command.Root>
            <Command.Input placeholder="Search language..." />

            <!-- Auto Detect — always visible, outside the scrollable/filtered list -->
            <div class="px-1 pt-1">
                <button
                    onclick={() => selectLanguage("auto")}
                    class="flex w-full items-center gap-2 rounded-sm px-2 py-2 text-sm cursor-default
                           hover:bg-accent hover:text-accent-foreground transition-colors
                           {language === 'auto' ? 'bg-accent/50 text-accent-foreground' : ''}"
                >
                    <span class="w-4 shrink-0 flex items-center justify-center">
                        {#if language === "auto"}
                            <Check class="w-4 h-4" strokeWidth={2.5} />
                        {/if}
                    </span>
                    {#if detectedLangMeta?.icon}
                        {#if "svg" in detectedLangMeta.icon}
                            <div class="w-4 h-4 flex items-center justify-center shrink-0 [&_svg]:size-full [&_svg]:block" style="fill: currentColor;">
                                {@html detectedLangMeta.icon.svg}
                            </div>
                        {:else}
                            {@const Icon = detectedLangMeta.icon}
                            <Icon class="w-4 h-4 shrink-0" strokeWidth={2.5} />
                        {/if}
                    {:else}
                        <div class="w-4 h-4 shrink-0"></div>
                    {/if}
                    <span class="flex-1 text-left">Auto Detect</span>
                    {#if detectedLangMeta}
                        <span class="text-xs text-muted-foreground">{detectedLangMeta.label}</span>
                    {/if}
                </button>
            </div>

            <Command.Separator />

            <!-- Language list — scrollable & filterable -->
            <Command.List class="max-h-[300px] overscroll-none">
                <Command.Empty>No language found.</Command.Empty>
                <div class="p-1">
                    {#each languageItems as lang (lang.value)}
                        {@const isActive = lang.value === language}
                        <Command.Item
                            value={lang.value}
                            keywords={[lang.label, lang.value]}
                            onSelect={() => selectLanguage(lang.value)}
                            class="flex w-full items-center gap-2 text-[13px]"
                        >
                            <span class="w-4 shrink-0 flex items-center justify-center">
                                {#if isActive}
                                    <Check class="w-4 h-4" strokeWidth={2.5} />
                                {/if}
                            </span>
                            {#if lang.icon}
                                {#if "svg" in lang.icon}
                                    <div class="w-4 h-4 flex items-center justify-center shrink-0 [&_svg]:size-full [&_svg]:block" style="fill: currentColor;">
                                        {@html lang.icon.svg}
                                    </div>
                                {:else}
                                    {@const Icon = lang.icon}
                                    <Icon class="w-4 h-4 shrink-0" strokeWidth={2.5} />
                                {/if}
                            {:else}
                                <div class="w-4 h-4 shrink-0"></div>
                            {/if}
                            <span>{lang.label}</span>
                        </Command.Item>
                    {/each}
                </div>
            </Command.List>
        </Command.Root>
    </Dialog.Content>
</Dialog.Root>
