<script lang="ts">
    import { editorState } from "$lib/state/editor.svelte";
    import {
        editorFindNext,
        editorFindPrevious,
        editorReplaceNext,
        editorReplaceAll,
        editorSetSearchQuery,
        updateSearchStats,
    } from "$lib/editor/core/actions";
    import { Button } from "$lib/components/ui/button";
    import { Input } from "$lib/components/ui/input";
    import {
        ArrowUp,
        ArrowDown,
        X,
        ChevronDown,
        ChevronRight,
        Check,
    } from "@lucide/svelte";

    let findText = $state("");
    let replaceText = $state("");

    // Convenience aliases — avoids repeating long chains everywhere
    const fr = $derived(editorState.findReplace);
    const view = $derived(editorState.activeView);

    // Derived flags used for button disabled states
    const canNavigate = $derived(fr.matchCount > 1);
    const canReplace = $derived(fr.matchCount > 0 && replaceText.length > 0);
    const canReplaceAll = $derived(fr.matchCount > 1 && replaceText.length > 0);

    let findInputRef: HTMLInputElement | null = $state(null);

    // Sync global → local when panel first becomes visible
    $effect(() => {
        if (fr.visible) {
            findText = fr.findText;
            replaceText = fr.replaceText;

            // Focus and select the existing text inside the find box
            if (findInputRef) {
                // We use setTimeout so it happens after the DOM has fully rendered
                setTimeout(() => {
                    findInputRef?.focus();
                    findInputRef?.select();
                }, 10);
            }
        }
    });

    // Sync local → global and drive the CodeMirror search query reactively
    $effect(() => {
        fr.findText = findText;
        fr.replaceText = replaceText;

        if (view && fr.visible) {
            editorSetSearchQuery(view, findText, replaceText, false);
            updateSearchStats(view);
        }
    });

    function close() {
        fr.visible = false;
        view?.focus();
    }

    function toggleReplaceMode() {
        fr.replaceMode = !fr.replaceMode;
    }

    /** Find input: Enter = next, Shift+Enter = previous, Escape = close */
    function handleFindKeydown(e: KeyboardEvent) {
        if (e.key === "Enter") {
            e.preventDefault();
            if (e.shiftKey) editorFindPrevious(view, false);
            else editorFindNext(view, false);
        } else if (e.key === "Escape") {
            close();
        }
    }

    /** Replace input: Enter = replace next, Escape = close */
    function handleReplaceKeydown(e: KeyboardEvent) {
        if (e.key === "Enter") {
            e.preventDefault();
            editorReplaceNext(view, false);
        } else if (e.key === "Escape") {
            close();
        }
    }

    function handleWindowKeydown(e: KeyboardEvent) {
        if (e.key === "Escape" && fr.visible) close();
    }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

{#if editorState.findReplace.visible}
    <!-- Floating Find & Replace Panel -->
    <div
        class="absolute top-4 right-8 z-50 flex flex-col gap-2 rounded-md border border-border bg-popover p-2 shadow-md"
        role="dialog"
        aria-label="Find and Replace"
    >
        <!-- Find Row -->
        <div class="flex items-center gap-1">
            <Button
                variant="ghost"
                size="icon"
                class="h-6 w-6 shrink-0"
                onclick={toggleReplaceMode}
                title="Toggle Replace"
            >
                {#if fr.replaceMode}
                    <ChevronDown class="h-4 w-4" />
                {:else}
                    <ChevronRight class="h-4 w-4" />
                {/if}
            </Button>
            <div class="relative flex items-center">
                <Input
                    bind:ref={findInputRef}
                    bind:value={findText}
                    onkeydown={handleFindKeydown}
                    placeholder="Find"
                    class="h-7 w-52 text-xs placeholder:text-muted-foreground/50 border-input bg-transparent pr-14"
                    spellcheck="false"
                />
                {#if findText.length > 0}
                    <div
                        class="absolute right-2 text-[10px] text-muted-foreground pointer-events-none"
                    >
                        {#if fr.matchCount > 0}
                            {fr.currentMatch} of {fr.matchCount}
                        {:else}
                            No results
                        {/if}
                    </div>
                {/if}
            </div>
            <div class="flex items-center border-l pl-1 ml-1">
                <Button
                    variant="ghost"
                    size="icon"
                    class="h-6 w-6"
                    onclick={() => editorFindPrevious(view, false)}
                    title="Previous match (Shift+Enter)"
                    disabled={!canNavigate}
                >
                    <ArrowUp class="h-4 w-4" />
                </Button>
                <Button
                    variant="ghost"
                    size="icon"
                    class="h-6 w-6"
                    onclick={() => editorFindNext(view, false)}
                    title="Next match (Enter)"
                    disabled={!canNavigate}
                >
                    <ArrowDown class="h-4 w-4" />
                </Button>
            </div>
            <Button
                variant="ghost"
                size="icon"
                class="h-6 w-6 ml-1"
                onclick={close}
                title="Close (Escape)"
            >
                <X class="h-4 w-4" />
            </Button>
        </div>

        <!-- Replace Row -->
        {#if fr.replaceMode}
            <div class="flex items-center gap-1 pl-7">
                <Input
                    bind:value={replaceText}
                    onkeydown={handleReplaceKeydown}
                    placeholder="Replace"
                    class="h-7 w-52 text-xs placeholder:text-muted-foreground/50 border-input bg-transparent"
                    spellcheck="false"
                />
                <div class="flex items-center border-l pl-1 ml-1 gap-1">
                    <Button
                        variant="ghost"
                        size="icon"
                        class="h-6 w-6"
                        onclick={() => editorReplaceNext(view, false)}
                        title="Replace currently selected match"
                        disabled={!canReplace}
                    >
                        <Check class="h-4 w-4" />
                    </Button>
                    <Button
                        variant="ghost"
                        class="h-6 px-2 text-xs"
                        onclick={() => editorReplaceAll(view, false)}
                        title="Replace All matches"
                        disabled={!canReplaceAll}
                    >
                        All
                    </Button>
                </div>
            </div>
        {/if}
    </div>
{/if}
