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
                        variant="secondary"
                        size="sm"
                        class="h-8 px-2 gap-1.5 text-xs font-medium bg-muted/50 hover:bg-muted"
                        {...props}
                        onclick={(e) => {
                            editorState.csv.showTable = false;
                            if (typeof props.onclick === "function") {
                                props.onclick(e);
                            }
                        }}
                    >
                        <FileText class="w-4 h-4" />
                        Plain CSV
                    </Button>
                {:else}
                    <!-- In Text Mode: Show button to switch to Table -->
                    <Button
                        variant="ghost"
                        size="sm"
                        class="h-8 px-2 gap-1.5 text-xs font-medium text-muted-foreground hover:bg-muted/50 hover:text-foreground"
                        {...props}
                        onclick={(e) => {
                            editorState.csv.showTable = true;
                            if (typeof props.onclick === "function") {
                                props.onclick(e);
                            }
                        }}
                    >
                        <Table2 class="w-4 h-4" />
                        Table
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
                    variant={editorState.markdown.showPreview
                        ? "secondary"
                        : "ghost"}
                    size="sm"
                    class="h-8 px-2 gap-1.5 text-xs font-medium {editorState
                        .markdown.showPreview
                        ? 'bg-primary/10 text-primary hover:bg-primary/20'
                        : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
                    {...props}
                    onclick={(e) => {
                        editorState.markdown.showPreview =
                            !editorState.markdown.showPreview;
                        if (typeof props.onclick === "function") {
                            props.onclick(e);
                        }
                    }}
                >
                    <Eye class="w-4 h-4" />
                    Preview
                </Button>
            {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content side="bottom">
            {editorState.markdown.showPreview ? "Hide preview" : "Show preview"}
        </Tooltip.Content>
    </Tooltip.Root>
{/if}
