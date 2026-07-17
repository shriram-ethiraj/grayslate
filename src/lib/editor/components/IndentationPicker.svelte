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

  // Raw, per-document indentation preference. Unlike `IndentConfig`, this can
  // hold the literal "default" mode — meaning "follow the global default
  // indentation setting" — as a real, persisted choice rather than a one-time
  // copy of concrete values. Callers resolve "default" to a concrete
  // `IndentConfig` at the point of use (see `effectiveIndentConfig` in
  // EditorWrapper.svelte).
  export type IndentSelection = {
    indentMode: IndentMode | "default";
    indentSize: number;
  };
</script>

<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { invoke } from "@tauri-apps/api/core";
  import { DEFAULT_INDENT_CONFIG } from "$lib/editor/core/editorSession";
  import { appSettingsState } from "$lib/state/appSettings.svelte";

  let {
    open = $bindable(false),
    indentSelection = $bindable<IndentSelection>(DEFAULT_INDENT_CONFIG),
    content = "",
  }: {
    open: boolean;
    indentSelection: IndentSelection;
    content?: string;
  } = $props();

  // "Default" and "Detect from content" are not persisted modes — selecting
  // either resolves immediately into a concrete Spaces/Tab + size pair.
  // "Default" resets to the user's global default (Settings → General →
  // Default indentation), keeping this picker's "Default" in lock-step with
  // the same value that seeds new documents. "Detect" triggers a one-shot
  // backend scan, matching VSCode's one-shot `detectIndentation`.
  const defaultValue = "default";
  const detectValue = "detect";
  const defaultSizeLabel = $derived(
    appSettingsState.defaultIndentMode === IndentMode.Tab
      ? `${appSettingsState.defaultIndentSize}-wide tab`
      : `${appSettingsState.defaultIndentSize} space${appSettingsState.defaultIndentSize === 1 ? "" : "s"}`,
  );
  const indentOptions = $derived([
    {
      value: defaultValue,
      label: `Default (${defaultSizeLabel})`,
    },
    { value: IndentMode.Spaces, label: "Spaces" },
    { value: IndentMode.Tab, label: "Tab" },
    { value: detectValue, label: "Detect from content" },
  ]);

  const spaceSizeOptions = Array.from({ length: 8 }, (_, i) => ({
    value: String(i + 1),
    label: String(i + 1),
  }));

  const activeModeLabel = $derived(
    indentOptions.find((o) => o.value === indentSelection.indentMode)?.label ?? "Spaces",
  );

  const sizeLabel = $derived(
    indentSelection.indentMode === IndentMode.Tab ? "Tab Size" : "Indent Size",
  );

  async function handleModeChange(value: string) {
    if (value === defaultValue) {
      indentSelection.indentMode = "default";
      return;
    }

    if (value !== detectValue) {
      indentSelection.indentMode = value as IndentMode;
      return;
    }

    open = false;

    const result = await invoke<{ useTabs: boolean; width: number }>("editor_detect_indent", {
      content,
    });
    indentSelection.indentMode = result.useTabs ? IndentMode.Tab : IndentMode.Spaces;
    indentSelection.indentSize = result.width;
  }

  function handleSizeChange(value: string) {
    indentSelection.indentSize = Number(value);
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content data-testid="indent-picker" class="sm:max-w-88" showCloseButton={true}>
    <div class="grid gap-4">
      <div class="grid gap-2">
        <label class="text-sm font-normal text-foreground" for="indent-mode-select">
          Indentation
        </label>
        <Select.Root type="single" value={indentSelection.indentMode} onValueChange={handleModeChange}>
          <Select.Trigger data-testid="indent-mode-trigger" class="w-full" id="indent-mode-select">
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

      {#if indentSelection.indentMode === IndentMode.Spaces || indentSelection.indentMode === IndentMode.Tab}
        <div class="grid gap-2">
          <label class="text-sm font-normal text-foreground" for="indent-size-select">
            {sizeLabel}
          </label>
          <Select.Root
            type="single"
            value={String(indentSelection.indentSize)}
            onValueChange={handleSizeChange}
          >
            <Select.Trigger data-testid="indent-size-trigger" class="w-full" id="indent-size-select">
              {indentSelection.indentSize}
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
