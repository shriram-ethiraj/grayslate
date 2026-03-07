<script lang="ts">
    import { onDestroy } from "svelte";
    import { editorState } from "$lib/state/editor.svelte";
    import { Button } from "$lib/components/ui/button/index.js";
    import Table2 from "~icons/lucide/table-2";
    import FileText from "~icons/lucide/file-text";
    import Eye from "~icons/lucide/eye";
    import Copy from "~icons/lucide/copy";
    import Check from "~icons/lucide/check";
    import { editorCopySelectionOrAll } from "$lib/editor/core/actions";
    import {
        copyMarkdownPreviewSelectionOrAll,
    } from "$lib/editor/components/markdown/previewActions";

    const COPY_SUCCESS_DURATION_MS = 1200;

    let showCopySuccess = $state(false);
    let copySuccessTimer: ReturnType<typeof setTimeout> | undefined;

    const showCopyAction = $derived(
        !(editorState.fileType === "csv" && editorState.csv.showTable),
    );

    const isCopyDisabled = $derived.by(() => {
        if (editorState.loader.visible) {
            return true;
        }

        return editorState.currentDocumentLength === 0;
    });

    const copyTitle = $derived.by(() => {
        if (showCopySuccess) {
            return "Copied";
        }

        if (isCopyDisabled) {
            return "Nothing to copy";
        }

        if (editorState.currentSelectionSize > 0) {
            return "Copy selection";
        }

        if (
            editorState.fileType === "markdown" &&
            editorState.markdown.showPreview &&
            editorState.activeSurface === "markdown-preview"
        ) {
            return "Copy preview text";
        }

        if (editorState.markdown.showPreview && editorState.fileType === "markdown") {
            return "Copy markdown source";
        }

        return "Copy all content";
    });

    function resetCopySuccessTimer() {
        if (copySuccessTimer !== undefined) {
            clearTimeout(copySuccessTimer);
            copySuccessTimer = undefined;
        }
    }

    function showCopySuccessState() {
        showCopySuccess = true;
        resetCopySuccessTimer();
        copySuccessTimer = setTimeout(() => {
            showCopySuccess = false;
            copySuccessTimer = undefined;
        }, COPY_SUCCESS_DURATION_MS);
    }

    async function handleCopyContent() {
        let copied = false;

        if (
            editorState.fileType === "markdown" &&
            editorState.markdown.showPreview &&
            editorState.activeSurface === "markdown-preview"
        ) {
            copied = await copyMarkdownPreviewSelectionOrAll();
        } else {
            copied = await editorCopySelectionOrAll(editorState.activeView);
        }

        if (copied) {
            showCopySuccessState();
        }
    }

    onDestroy(() => {
        resetCopySuccessTimer();
    });
</script>

{#if showCopyAction}
    <Button
        variant="ghost"
        size="icon"
        aria-label="Copy content"
        title={copyTitle}
        disabled={isCopyDisabled}
        onclick={() => {
            void handleCopyContent();
        }}
    >
        {#if showCopySuccess}
            <Check class="h-[1.2rem] w-[1.2rem] transition-all" />
        {:else}
            <Copy class="h-[1.2rem] w-[1.2rem] transition-all" />
        {/if}
    </Button>
{/if}

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
                if (editorState.csv.requestShowTable) {
                    void editorState.csv.requestShowTable(false);
                } else {
                    editorState.csv.showTable = false;
                }
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
                if (editorState.csv.requestShowTable) {
                    void editorState.csv.requestShowTable(true);
                } else {
                    editorState.csv.showTable = true;
                }
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
