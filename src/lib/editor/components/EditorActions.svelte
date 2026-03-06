<script lang="ts">
    import { editorState } from "$lib/state/editor.svelte";
    import { Button } from "$lib/components/ui/button/index.js";
    import Table2 from "~icons/lucide/table-2";
    import FileText from "~icons/lucide/file-text";
    import Eye from "~icons/lucide/eye";
</script>

{#if editorState.fileType === "csv"}
    {#if editorState.csv.showTable}
        <!-- In Table Mode: Show button to switch to Plain CSV -->
        <Button
            variant="ghost"
            size="icon"
            aria-label="Plain CSV"
            title="Switch to Plain CSV"
            disabled={editorState.loader.visible}
            onclick={() => {
                editorState.csv.showTable = false;
            }}
        >
            <FileText class="h-[1.2rem] w-[1.2rem] transition-all" />
        </Button>
    {:else}
        <!-- In Text Mode: Show button to switch to Table -->
        <Button
            variant="ghost"
            size="icon"
            aria-label="Table View"
            title="Switch to Table View"
            disabled={editorState.loader.visible}
            onclick={() => {
                editorState.csv.showTable = true;
            }}
        >
            <Table2 class="h-[1.2rem] w-[1.2rem] transition-all" />
        </Button>
    {/if}
{/if}

{#if editorState.fileType === "markdown"}
    <!-- Markdown Preview toggle -->
    <Button
        variant="ghost"
        size="icon"
        aria-label="Toggle Preview"
        title={editorState.markdown.showPreview
            ? "Hide preview"
            : "Show preview"}
        disabled={editorState.loader.visible}
        class={editorState.markdown.showPreview
            ? "bg-accent text-accent-foreground"
            : ""}
        onclick={() => {
            editorState.markdown.showPreview =
                !editorState.markdown.showPreview;
        }}
    >
        <Eye class="h-[1.2rem] w-[1.2rem] transition-all" />
    </Button>
{/if}
