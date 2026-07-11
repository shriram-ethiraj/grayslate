<script module lang="ts">
  export const IndentMode = {
    Spaces: "spaces",
    Tab: "tab",
  } as const;

  export type IndentMode = (typeof IndentMode)[keyof typeof IndentMode];

  export type IndentConfig = {
    indentMode: IndentMode;
    indentSize: number;
  };
</script>

<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { invoke } from "@tauri-apps/api/core";
  import { DEFAULT_INDENT_CONFIG } from "$lib/editor/core/editorSession";

  let {
    open = $bindable(false),
    indentConfig = $bindable<IndentConfig>(DEFAULT_INDENT_CONFIG),
    content = "",
  }: {
    open: boolean;
    indentConfig: IndentConfig;
    content?: string;
  } = $props();

  // "Default" and "Detect from content" are not persisted modes — selecting
  // either resolves immediately into a concrete Spaces/Tab + size pair.
  // "Default" resets to DEFAULT_INDENT_CONFIG; "Detect" triggers a one-shot
  // backend scan, matching VSCode's one-shot `detectIndentation`.
  const defaultValue = "default";
  const detectValue = "detect";
  const defaultSizeLabel =
    DEFAULT_INDENT_CONFIG.indentMode === IndentMode.Tab
      ? `${DEFAULT_INDENT_CONFIG.indentSize}-wide tab`
      : `${DEFAULT_INDENT_CONFIG.indentSize} space${DEFAULT_INDENT_CONFIG.indentSize === 1 ? "" : "s"}`;
  const indentOptions = [
    {
      value: defaultValue,
      label: `Default (${defaultSizeLabel})`,
    },
    { value: IndentMode.Spaces, label: "Spaces" },
    { value: IndentMode.Tab, label: "Tab" },
    { value: detectValue, label: "Detect from content" },
  ];

  const spaceSizeOptions = Array.from({ length: 8 }, (_, i) => ({
    value: String(i + 1),
    label: String(i + 1),
  }));

  const activeModeLabel = $derived(
    indentOptions.find((o) => o.value === indentConfig.indentMode)?.label ?? "Spaces",
  );

  const sizeLabel = $derived(
    indentConfig.indentMode === IndentMode.Tab ? "Tab Size" : "Indent Size",
  );

  async function handleModeChange(value: string) {
    if (value === defaultValue) {
      indentConfig.indentMode = DEFAULT_INDENT_CONFIG.indentMode;
      indentConfig.indentSize = DEFAULT_INDENT_CONFIG.indentSize;
      return;
    }

    if (value !== detectValue) {
      indentConfig.indentMode = value as IndentMode;
      return;
    }

    open = false;

    const result = await invoke<{ useTabs: boolean; width: number }>("editor_detect_indent", {
      content,
    });
    indentConfig.indentMode = result.useTabs ? IndentMode.Tab : IndentMode.Spaces;
    indentConfig.indentSize = result.width;
  }

  function handleSizeChange(value: string) {
    indentConfig.indentSize = Number(value);
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-88" showCloseButton={true}>
    <div class="grid gap-4">
      <div class="grid gap-2">
        <label class="text-sm font-medium text-foreground" for="indent-mode-select">
          Indentation
        </label>
        <Select.Root type="single" value={indentConfig.indentMode} onValueChange={handleModeChange}>
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

      {#if indentConfig.indentMode === IndentMode.Spaces || indentConfig.indentMode === IndentMode.Tab}
        <div class="grid gap-2">
          <label class="text-sm font-medium text-foreground" for="indent-size-select">
            {sizeLabel}
          </label>
          <Select.Root
            type="single"
            value={String(indentConfig.indentSize)}
            onValueChange={handleSizeChange}
          >
            <Select.Trigger class="w-full" id="indent-size-select">
              {indentConfig.indentSize}
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
