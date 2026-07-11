<script lang="ts">
    import { toast } from "$lib/components/ui/sonner";
    import * as Dialog from "$lib/components/ui/dialog/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import Trash2 from "~icons/lucide/trash-2";
    import { appDialogsState, closeAppDialog } from "$lib/state/appDialogs.svelte";
    import { editorState } from "$lib/state/editor.svelte";
    import { performFileDelete } from "$lib/files/recentFiles";

    const isOpen = $derived(appDialogsState.active.type === "delete");
    const file = $derived(
        appDialogsState.active.type === "delete" ? appDialogsState.active.file : null,
    );
    const isCurrentFile = $derived(
        !!file && file.path === editorState.currentFilePath,
    );

    let isDeleting = $state(false);

    async function handleDelete(): Promise<void> {
        if (!file) return;

        isDeleting = true;
        try {
            await performFileDelete(file);
            closeAppDialog();
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            toast.error(`Failed to delete: ${msg}`);
        } finally {
            isDeleting = false;
        }
    }
</script>

<Dialog.Root
    open={isOpen}
    onOpenChange={(open) => {
        if (!open && !isDeleting) closeAppDialog();
    }}
>
    <Dialog.Content class="sm:max-w-[26rem]">
        <Dialog.Header>
            <Dialog.Title>Delete file?</Dialog.Title>
            <Dialog.Description>
                {#if file}
                    <span class="font-medium text-foreground">{file.file_name}</span>
                    {" "}will be permanently deleted and cannot be recovered.
                    {#if isCurrentFile}
                        <br /><br />
                        <span class="text-muted-foreground">
                            This file is currently open. The editor will reset to a new slate.
                        </span>
                    {/if}
                {/if}
            </Dialog.Description>
        </Dialog.Header>

        <Dialog.Footer>
            <Button
                variant="outline"
                onclick={closeAppDialog}
                disabled={isDeleting}
            >
                Cancel
            </Button>
            <Button
                variant="destructive"
                onclick={() => void handleDelete()}
                disabled={isDeleting}
            >
                {#if isDeleting}
                    Deleting…
                {:else}
                    <Trash2 class="size-4" />
                    Delete permanently
                {/if}
            </Button>
        </Dialog.Footer>
    </Dialog.Content>
</Dialog.Root>
