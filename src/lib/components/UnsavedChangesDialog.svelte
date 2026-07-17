<script lang="ts">
    import * as Dialog from "$lib/components/ui/dialog/index.js";
    import { Button } from "$lib/components/ui/button/index.js";
    import { appDialogsState, closeAppDialog } from "$lib/state/appDialogs.svelte";
    import { editorState } from "$lib/state/editor.svelte";

    const isOpen = $derived(appDialogsState.active.type === "unsaved-changes");
    const resolve = $derived(
        appDialogsState.active.type === "unsaved-changes"
            ? appDialogsState.active.resolve
            : null,
    );

    const fileName = $derived.by(() => {
        const path = editorState.currentFilePath;
        if (!path) return "this file";
        return path.replace(/\\/g, "/").split("/").pop() ?? path;
    });

    function handleChoice(choice: "save" | "discard" | "cancel"): void {
        resolve?.(choice);
    }
</script>

<Dialog.Root
    open={isOpen}
    onOpenChange={(open) => {
        if (!open) handleChoice("cancel");
    }}
>
    <Dialog.Content data-testid="unsaved-changes-dialog" class="sm:max-w-[26rem]">
        <Dialog.Header>
            <Dialog.Title>Save changes?</Dialog.Title>
            <Dialog.Description>
                Do you want to save the changes you made to{" "}
                <span class="font-medium text-foreground">{fileName}</span>?<br />
                Your changes will be lost if you don't save them.
            </Dialog.Description>
        </Dialog.Header>

        <Dialog.Footer>
            <Button
                variant="outline"
                onclick={() => handleChoice("cancel")}
                data-testid="unsaved-cancel"
            >
                Cancel
            </Button>
            <Button
                variant="outline"
                onclick={() => handleChoice("discard")}
                data-testid="unsaved-discard"
            >
                Discard
            </Button>
            <Button
                onclick={() => handleChoice("save")}
                data-testid="unsaved-save"
            >
                Save
            </Button>
        </Dialog.Footer>
    </Dialog.Content>
</Dialog.Root>
