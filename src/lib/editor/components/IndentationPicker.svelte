<script module lang="ts">
  export const IndentMode = {
    Detect: "detect",
    Default: "default",
    Spaces: "spaces",
    Tab: "tab",
  } as const;

  export type IndentMode = (typeof IndentMode)[keyof typeof IndentMode];
</script>

<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import * as Select from "$lib/components/ui/select/index.js";

  let {
    open = $bindable(false),
    indentMode = $bindable<IndentMode>(IndentMode.Default),
    indentSize = $bindable(2),
  }: {
    open: boolean;
    indentMode: IndentMode;
    indentSize: number;
  } = $props();

  const indentOptions = [
    { value: IndentMode.Detect, label: "Detect from content" },
    { value: IndentMode.Default, label: "Default (Spaces: 2)" },
    { value: IndentMode.Spaces, label: "Spaces" },
    { value: IndentMode.Tab, label: "Tab" },
  ];

  const spaceSizeOptions = Array.from({ length: 8 }, (_, i) => ({
    value: String(i + 1),
    label: String(i + 1),
  }));

  let sizeValue = $state(String(indentSize));

  $effect(() => {
    indentSize = Number(sizeValue);
  });

  const activeModeLabel = $derived(
    indentOptions.find((o) => o.value === indentMode)?.label ?? "Default (Spaces: 2)",
  );
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-88" showCloseButton={true}>
    <div class="grid gap-4">
      <div class="grid gap-2">
        <label class="text-sm font-medium text-foreground" for="indent-mode-select">
          Indentation
        </label>
        <Select.Root type="single" bind:value={indentMode}>
          <Select.Trigger class="w-full" id="indent-mode-select">
            {activeModeLabel}
          </Select.Trigger>
          <Select.Content>
            {#each indentOptions as option (option.value)}
              <Select.Item value={option.value} label={option.label}>
                {option.label}
              </Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      </div>

      {#if indentMode === IndentMode.Spaces}
        <div class="grid gap-2">
          <label class="text-sm font-medium text-foreground" for="indent-size-select">
            Tab Size
          </label>
          <Select.Root type="single" bind:value={sizeValue}>
            <Select.Trigger class="w-full" id="indent-size-select">
              {sizeValue}
            </Select.Trigger>
            <Select.Content>
              {#each spaceSizeOptions as option (option.value)}
                <Select.Item value={option.value} label={option.label}>
                  {option.label}
                </Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
        </div>
      {/if}
    </div>
  </Dialog.Content>
</Dialog.Root>
