<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import {
    CHARACTER_ENCODING_OPTIONS,
    type CharacterEncoding,
  } from "$lib/state/appSettings.svelte";

  let {
    encoding,
    reason,
    onConfirm,
    onCancel,
  }: {
    encoding: CharacterEncoding;
    reason: "legacySingleByte" | "bomlessUtf16";
    onConfirm: () => void;
    onCancel: () => void;
  } = $props();

  const label = $derived(
    CHARACTER_ENCODING_OPTIONS.find((option) => option.value === encoding)?.label ?? encoding,
  );
</script>

<Dialog.Root
  open={true}
  onOpenChange={(open) => {
    if (!open) onCancel();
  }}
>
  <Dialog.Content data-testid="encoding-confirmation-dialog" class="sm:max-w-96">
    <Dialog.Header>
      <Dialog.Title>Confirm character encoding</Dialog.Title>
      <Dialog.Description>
        {#if reason === "bomlessUtf16"}
          This looks like {label} without a byte-order mark. Reopen it with that encoding?
          Saving it later will add the standard byte-order mark.
        {:else}
          This is not valid UTF-8 and may be legacy text. Reopen it as {label}?
        {/if}
      </Dialog.Description>
    </Dialog.Header>
    <Dialog.Footer>
      <Button variant="outline" onclick={onCancel}>Cancel</Button>
      <Button data-testid="encoding-confirmation-accept" onclick={onConfirm}>
        Reopen as {label}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
