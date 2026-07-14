<script lang="ts">
    import { onDestroy } from "svelte";
    import {
        editorState,
        openTransformationsPalette,
    } from "$lib/state/editor.svelte";
    import { Button } from "$lib/components/ui/button/index.js";
    import Table2 from "~icons/lucide/table-2";
    import FileText from "~icons/lucide/file-text";
    import Eye from "~icons/lucide/eye";
    import Copy from "~icons/lucide/copy";
    import Check from "~icons/lucide/check";
    import Zap from "~icons/lucide/zap";
    import Save from "~icons/lucide/save";
    import { editorCopySelectionOrAll } from "$lib/editor/core/actions";
    import { copyMarkdownPreviewSelectionOrAll } from "$lib/editor/components/markdown/previewActions";
    import { emit } from "@tauri-apps/api/event";

    const COPY_SUCCESS_DURATION_MS = 1200;

    let showCopySuccess = $state(false);
    let copySuccessTimer: ReturnType<typeof setTimeout> | undefined;

    const isCopyDisabled = $derived.by(() => {
        if (
            editorState.loader.visible ||
            (editorState.fileType === "csv" && editorState.csv.showTable) ||
            editorState.currentDocumentLength === 0
        ) {
            return true;
        }
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

        if (
            editorState.markdown.showPreview &&
            editorState.fileType === "markdown"
        ) {
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
            <FileText class="size-4 transition-all" />
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
            <Table2 class="size-4 transition-all" />
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
        aria-pressed={editorState.markdown.showPreview}
        onclick={() => {
            editorState.markdown.showPreview =
                !editorState.markdown.showPreview;
        }}
    >
        <Eye class="size-4 transition-all" />
    </Button>
{/if}

{#if editorState.currentFileSource === "local"}
    <Button
        variant="ghost"
        size="icon"
        aria-label="Save file"
        title="Save (Ctrl+S)"
        disabled={editorState.loader.visible || !editorState.isDirty}
        onclick={() => {
            void emit("menu://save-file");
        }}
    >
        <Save class="size-4" />
    </Button>
{/if}

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
        <Check class="size-4 transition-all" />
    {:else}
        <Copy class="size-4 transition-all" />
    {/if}
</Button>

<Button
    variant="ghost"
    size="icon"
    aria-label="Transformations"
    title="Open transformations"
    disabled={editorState.loader.visible}
    onclick={() => {
        openTransformationsPalette();
    }}
>
    <Zap class="size-4 transition-all" />
</Button>
