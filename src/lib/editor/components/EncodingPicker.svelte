<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { AppTooltip } from "$lib/components/ui/tooltip/index.js";
  import {
    CHARACTER_ENCODING_OPTIONS,
    type CharacterEncoding,
  } from "$lib/state/appSettings.svelte";
  import {
    openEncodingPicker,
    registerEditorPopup,
    syncEditorPopupOpenState,
  } from "$lib/state/editor.svelte";

  let {
    encoding = "utf-8",
    canReopen = false,
    reopenDisabledReason = "Save or discard your changes before reopening.",
    onReopen = async () => false,
    onSave = async () => false,
  }: {
    encoding?: CharacterEncoding;
    canReopen?: boolean;
    reopenDisabledReason?: string;
    onReopen?: (next: CharacterEncoding) => Promise<boolean>;
    onSave?: (next: CharacterEncoding) => Promise<boolean>;
  } = $props();

  let open = $state(false);
  let selected = $state<CharacterEncoding>("utf-8");
  let actionInProgress = $state(false);

  const activeLabel = $derived(
    CHARACTER_ENCODING_OPTIONS.find((option) => option.value === selected)?.label ?? "UTF-8",
  );

  async function runAction(action: "reopen" | "save"): Promise<void> {
    if (actionInProgress) return;
    actionInProgress = true;
    try {
      const succeeded = action === "reopen"
        ? await onReopen(selected)
        : await onSave(selected);
      if (succeeded) open = false;
    } finally {
      actionInProgress = false;
    }
  }

  $effect(() => {
    if (!open) selected = encoding;
    syncEditorPopupOpenState("encoding-picker", open);
  });

  $effect(() => {
    return registerEditorPopup("encoding-picker", {
      open: (request) => {
        if (request.id !== "encoding-picker") return;
        selected = encoding;
        open = true;
      },
      close: () => {
        open = false;
      },
    });
  });
</script>

<AppTooltip content="Select character encoding">
  {#snippet trigger({ props })}
    <button
      {...props}
      type="button"
      onclick={openEncodingPicker}
      class="ui-state flex h-full cursor-pointer items-center rounded-none px-2 text-xs"
      data-testid="status-encoding"
      data-encoding={encoding}
    >
      {CHARACTER_ENCODING_OPTIONS.find((option) => option.value === encoding)?.shortLabel ?? "UTF-8"}
    </button>
  {/snippet}
</AppTooltip>

<Dialog.Root bind:open>
  <Dialog.Content data-testid="encoding-picker-dialog" class="sm:max-w-96">
    <Dialog.Header class="gap-0.5 pr-8 text-left">
      <Dialog.Title class="text-sm font-normal leading-normal">
        Character encoding
      </Dialog.Title>
      <Dialog.Description class="text-xs leading-normal">
        Reopen interprets the original bytes again. Save converts the current text immediately.
      </Dialog.Description>
    </Dialog.Header>

    <Select.Root
      type="single"
      value={selected}
      onValueChange={(value) => {
        const option = CHARACTER_ENCODING_OPTIONS.find((candidate) => candidate.value === value);
        if (option) selected = option.value;
      }}
    >
      <Select.Trigger data-testid="encoding-select-trigger" class="w-full" aria-label="Character encoding">
        {activeLabel}
      </Select.Trigger>
      <Select.Content>
        {#each CHARACTER_ENCODING_OPTIONS as option (option.value)}
          <Select.Item
            value={option.value}
            label={option.label}
            data-testid={`encoding-item-${option.value}`}
          >
            {option.label}
          </Select.Item>
        {/each}
      </Select.Content>
    </Select.Root>

    <Dialog.Footer class="gap-2 sm:justify-between">
      <div class="grid gap-1">
        <Button
          variant="outline"
          data-testid="encoding-reopen"
          disabled={!canReopen || actionInProgress}
          onclick={() => void runAction("reopen")}
        >
          Reopen with Encoding
        </Button>
        {#if !canReopen}
          <span class="text-xs text-muted-foreground">{reopenDisabledReason}</span>
        {/if}
      </div>
      <Button
        data-testid="encoding-save"
        disabled={actionInProgress}
        onclick={() => void runAction("save")}
      >
        Save with Encoding
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
