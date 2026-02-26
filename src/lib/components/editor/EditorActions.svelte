<script lang="ts">
    import { editorState } from "$lib/state/editor.svelte";
    import { Button } from "$lib/components/ui/button/index.js";
    import * as Tooltip from "$lib/components/ui/tooltip/index.js";
    import { Table2, FileText, Eye } from "@lucide/svelte";
</script>

{#if editorState.fileType === "csv"}
    <Tooltip.Root>
        <Tooltip.Trigger>
            {#snippet child({ props }: { props: Record<string, unknown> })}
                {#if editorState.csv.showTable}
                    <!-- In Table Mode: Show button to switch to Plain CSV -->
                    <Button
                        variant="ghost"
                        size="icon"
                        aria-label="Plain CSV"
                        {...props}
                        onclick={(e) => {
                            editorState.csv.showTable = false;
                            if (typeof props.onclick === "function") {
                                props.onclick(e);
                            }
                        }}
                    >
                        <FileText
                            class="h-[1.2rem] w-[1.2rem] transition-all"
                        />
                    </Button>
                {:else}
                    <!-- In Text Mode: Show button to switch to Table -->
                    <Button
                        variant="ghost"
                        size="icon"
                        aria-label="Table View"
                        {...props}
                        onclick={(e) => {
                            editorState.csv.showTable = true;
                            if (typeof props.onclick === "function") {
                                props.onclick(e);
                            }
                        }}
                    >
                        <Table2 class="h-[1.2rem] w-[1.2rem] transition-all" />
                    </Button>
                {/if}
            {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content side="bottom">
            {editorState.csv.showTable
                ? "Switch to Plain CSV"
                : "Switch to Table View"}
        </Tooltip.Content>
    </Tooltip.Root>
{/if}

{#if editorState.fileType === "markdown"}
    <!-- Markdown Preview toggle -->
    <Tooltip.Root>
        <Tooltip.Trigger>
            {#snippet child({ props }: { props: Record<string, unknown> })}
                <Button
                    variant="ghost"
                    size="icon"
                    aria-label="Toggle Preview"
                    class={editorState.markdown.showPreview
                        ? "bg-accent text-accent-foreground"
                        : ""}
                    {...props}
                    onclick={(e) => {
                        editorState.markdown.showPreview =
                            !editorState.markdown.showPreview;
                        if (typeof props.onclick === "function") {
                            props.onclick(e);
                        }
                    }}
                >
                    <Eye class="h-[1.2rem] w-[1.2rem] transition-all" />
                </Button>
            {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content side="bottom">
            {editorState.markdown.showPreview ? "Hide preview" : "Show preview"}
        </Tooltip.Content>
    </Tooltip.Root>
{/if}
