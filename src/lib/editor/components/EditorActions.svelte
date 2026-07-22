<script lang="ts">
    import { onDestroy } from "svelte";
    import {
        editorState,
        openTransformationsPalette,
    } from "$lib/state/editor.svelte";
    import { TooltipButton } from "$lib/components/ui/tooltip/index.js";
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
    import { formatShortcutTooltip } from "$lib/shortcuts";
    import { platformState } from "$lib/state/platform.svelte";

    const COPY_SUCCESS_DURATION_MS = 1200;

    let showCopySuccess = $state(false);
    let copySuccessTimer: ReturnType<typeof setTimeout> | undefined;

    const isCsvTableMode = $derived(
        editorState.fileType === "csv" && editorState.csv.showTable,
    );

    const isCopyDisabled = $derived.by(() => {
        if (editorState.copyInProgress) return true;
        if (editorState.loader.visible) return true;
        if (isCsvTableMode) return editorState.csv.copy === undefined;
        return editorState.currentDocumentLength === 0;
    });

    const isSaveDisabled = $derived(
        editorState.loader.visible || editorState.saveInProgress || !editorState.isDirty,
    );

    const saveDisabledTooltip = $derived.by(() => {
        if (editorState.saveInProgress) return "Saving…";
        if (editorState.loader.visible) return "Unavailable while loading";
        return "No changes to save";
    });

    const transformationsDisabledTooltip = $derived(
        isCsvTableMode
            ? "Not available in CSV table mode"
            : "Unavailable while loading",
    );

    const copyTitle = $derived.by(() => {
        if (editorState.copyInProgress) {
            return "Copy in progress";
        }

        if (showCopySuccess) {
            return "Copied";
        }

        if (editorState.loader.visible) return "Unavailable while loading";
        if (isCsvTableMode) {
            return formatShortcutTooltip("Copy CSV content", "copy", platformState.osType);
        }
        if (editorState.currentDocumentLength === 0) return "Nothing to copy";

        let label = "Copy all content";

        if (editorState.currentSelectionSize > 0) {
            label = "Copy selection";
        } else if (
            editorState.fileType === "markdown" &&
            editorState.markdown.showPreview &&
            editorState.activeSurface === "markdown-preview"
        ) {
            label = "Copy preview text";
        } else if (
            editorState.markdown.showPreview &&
            editorState.fileType === "markdown"
        ) {
            label = "Copy markdown source";
        }

        return formatShortcutTooltip(label, "copy", platformState.osType);
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

        if (isCsvTableMode) {
            copied = (await editorState.csv.copy?.()) ?? false;
        } else if (
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
        <TooltipButton
            variant="ghost"
            size="icon"
            data-testid="action-plain-csv"
            aria-label="Plain CSV"
            tooltip="Switch to plain CSV"
            disabledTooltip="Unavailable while loading"
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
        </TooltipButton>
    {:else}
        <!-- In Text Mode: Show button to switch to Table -->
        <TooltipButton
            variant="ghost"
            size="icon"
            data-testid="action-table-view"
            aria-label="Table View"
            tooltip="Switch to table view"
            disabledTooltip="Unavailable while loading"
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
        </TooltipButton>
    {/if}
{/if}

{#if editorState.fileType === "markdown"}
    <!-- Markdown Preview toggle -->
    <TooltipButton
        variant="ghost"
        size="icon"
        data-testid="action-toggle-preview"
        aria-label="Toggle Preview"
        tooltip={editorState.markdown.showPreview
            ? "Hide preview"
            : "Show preview"}
        disabledTooltip="Unavailable while loading"
        disabled={editorState.loader.visible}
        aria-pressed={editorState.markdown.showPreview}
        onclick={() => {
            editorState.markdown.showPreview =
                !editorState.markdown.showPreview;
        }}
    >
        <Eye class="size-4 transition-all" />
    </TooltipButton>
{/if}

{#if editorState.currentFileSource === "local"}
    <TooltipButton
        variant="ghost"
        size="icon"
        data-testid="action-save"
        aria-label="Save file"
        tooltip={formatShortcutTooltip("Save", "save-file", platformState.osType)}
        disabledTooltip={saveDisabledTooltip}
        disabled={isSaveDisabled}
        onclick={() => {
            void emit("menu://save-file");
        }}
    >
        <Save class="size-4" />
    </TooltipButton>
{/if}

<TooltipButton
    variant="ghost"
    size="icon"
    data-testid="action-copy"
    aria-label="Copy content"
    tooltip={copyTitle}
    disabledTooltip={copyTitle}
    disabled={isCopyDisabled}
    onclick={() => {
        void handleCopyContent();
    }}
>
    {#if showCopySuccess && !editorState.copyInProgress}
        <Check class="size-4 transition-all" />
    {:else}
        <Copy class="size-4 transition-all" />
    {/if}
</TooltipButton>

<TooltipButton
    variant="ghost"
    size="icon"
    data-testid="action-transformations"
    aria-label="Transformations"
    tooltip={formatShortcutTooltip("Open transformations", "transformations", platformState.osType)}
    disabledTooltip={transformationsDisabledTooltip}
    disabled={editorState.loader.visible || isCsvTableMode}
    onclick={() => {
        openTransformationsPalette();
    }}
>
    <Zap class="size-4 transition-all" />
</TooltipButton>
