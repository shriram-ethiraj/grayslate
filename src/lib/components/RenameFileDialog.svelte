<script lang="ts">
    import { tick } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { toast } from "$lib/components/ui/sonner";
    import * as Dialog from "$lib/components/ui/dialog/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import Input from "$lib/components/ui/input/input.svelte";
    import MdiLightbulbAutomaticOutline from "~icons/mdi/lightbulb-automatic-outline";
    import {
        appDialogsState,
        closeAppDialog,
    } from "$lib/state/appDialogs.svelte";
    import { editorState } from "$lib/state/editor.svelte";
    import { librarySidebarState } from "$lib/state/librarySidebar.svelte";
    import {
        renameFile,
    } from "$lib/files/recentFiles";

    const isOpen = $derived(appDialogsState.active.type === "rename");
    const file = $derived(
        appDialogsState.active.type === "rename"
            ? appDialogsState.active.file
            : null,
    );

    let inputValue = $state("");
    let errorMessage = $state("");
    let isRenaming = $state(false);
    let isGenerating = $state(false);
    let inputRef = $state<HTMLInputElement | null>(null);

    // Pre-fill with the current filename whenever the dialog opens.
    $effect(() => {
        if (!isOpen) return;
        inputValue = file?.file_name ?? "";
        errorMessage = "";
        void focusAndSelectStem();
    });

    async function focusAndSelectStem(): Promise<void> {
        await tick();
        if (!inputRef) return;
        inputRef.focus();
        // Select just the stem so the user can type a new name without removing
        // the extension by accident.
        const dotPos = inputValue.lastIndexOf(".");
        inputRef.setSelectionRange(0, dotPos > 0 ? dotPos : inputValue.length);
    }

    async function handleGenerateName(): Promise<void> {
        if (!file) return;
        isGenerating = true;
        try {
            let suggested: string;

            // For the currently open file, use live editor content so that
            // unsaved changes are reflected in the suggestion — same as untitled save.
            const isCurrentFile = file.path === editorState.currentFilePath;
            const view = editorState.activeView;

            if (isCurrentFile && view) {
                const content = view.state.doc.sliceString(0, 8192);
                const result = await invoke<{
                    filename: string;
                    detectedLanguage: string;
                }>("suggest_slate_name", { content, languageHint: "auto" });
                suggested = result.filename;
            } else {
                // Non-current files: backend reads from disk and detects from content.
                suggested = await invoke<string>("suggest_name_for_file", {
                    path: file.path,
                });
            }

            inputValue = suggested;
            errorMessage = "";
            await focusAndSelectStem();
        } catch (err) {
            toast.error("Failed to generate name.");
        } finally {
            isGenerating = false;
        }
    }

    async function handleSubmit(): Promise<void> {
        if (!file) return;

        const trimmed = inputValue.trim();
        if (!trimmed) {
            errorMessage = "File name cannot be empty.";
            return;
        }
        if (trimmed.includes("/") || trimmed.includes("\\")) {
            errorMessage = "File name cannot contain path separators.";
            return;
        }

        isRenaming = true;
        const oldPath = file.path;
        const wasCurrentFile = oldPath === editorState.currentFilePath;

        try {
            const newPath = await renameFile(oldPath, trimmed);
            closeAppDialog();

            // Refresh sidebar data to pick up the new filename. This triggers
            // a quiet refetch that updates file metadata in place without
            // clearing suppression — so the list doesn't reorder.
            librarySidebarState.requestQuietDataRefresh?.();

            // Keep the editor's save path in sync with the renamed file.
            // Signal the sidebar BEFORE updating the path so it can update
            // its suppression tracking instead of misinterpreting the path
            // change as an external navigation.
            if (wasCurrentFile) {
                librarySidebarState.lastRenamedPath = { from: oldPath, to: newPath };
                editorState.currentFilePath = newPath;
            }

            const newName =
                newPath.replace(/\\/g, "/").split("/").pop() ?? trimmed;
            toast.success(`Renamed to "${newName}"`);
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            errorMessage = msg;
        } finally {
            isRenaming = false;
        }
    }
</script>

<Dialog.Root
    open={isOpen}
    onOpenChange={(open) => {
        if (!open && !isRenaming) closeAppDialog();
    }}
>
    <Dialog.Content
        class="sm:max-w-[25rem]"
        onOpenAutoFocus={(event) => {
            event.preventDefault();
            void focusAndSelectStem();
        }}
    >
        <form
            class="grid gap-3"
            onsubmit={(event) => {
                event.preventDefault();
                void handleSubmit();
            }}
        >
            <div class="grid gap-2">
                <label
                    class="text-sm font-medium text-foreground"
                    for="rename-file-input"
                >
                    Rename file
                </label>
                <Input
                    id="rename-file-input"
                    bind:ref={inputRef}
                    bind:value={inputValue}
                    type="text"
                    autocomplete="off"
                    spellcheck="false"
                    aria-invalid={errorMessage.length > 0}
                    aria-describedby="rename-file-error"
                    disabled={isRenaming}
                    oninput={() => {
                        errorMessage = "";
                    }}
                />
                {#if errorMessage.length > 0}
                    <p id="rename-file-error" class="text-xs text-destructive">
                        {errorMessage}
                    </p>
                {/if}
            </div>

            <Dialog.Footer class="flex-col gap-2 sm:flex-row sm:items-center">
                <Button
                    type="button"
                    variant="outline"
                    class="w-full gap-1.5 sm:mr-auto sm:w-auto"
                    disabled={isRenaming || isGenerating}
                    onclick={() => void handleGenerateName()}
                >
                    <MdiLightbulbAutomaticOutline class="size-3.5" />
                    Generate name
                </Button>
                <Button
                    type="button"
                    variant="outline"
                    onclick={closeAppDialog}
                    disabled={isRenaming || isGenerating}
                >
                    Cancel
                </Button>
                <Button type="submit" disabled={isRenaming || isGenerating}>
                    {isRenaming ? "Renaming…" : "Rename"}
                </Button>
            </Dialog.Footer>
        </form>
    </Dialog.Content>
</Dialog.Root>
