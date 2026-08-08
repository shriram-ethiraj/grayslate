<script lang="ts">
  import LanguagePicker from "./LanguagePicker.svelte";
  import EolPicker from "./EolPicker.svelte";
  import EncodingPicker from "./EncodingPicker.svelte";
  import type { CharacterEncoding, Eol } from "$lib/state/appSettings.svelte";
  import { IndentMode, type IndentConfig } from "./IndentationPicker.svelte";
  import { DEFAULT_INDENT_CONFIG } from "$lib/editor/core/editorSession";
  import { AppTooltip } from "$lib/components/ui/tooltip/index.js";
  import { formatShortcutTooltip } from "$lib/shortcuts";
  import { platformState } from "$lib/state/platform.svelte";
  import { appMenuState, openAboutDialog } from "$lib/state/appMenu.svelte";
  import Download from "~icons/lucide/download";
  import LoaderCircle from "~icons/lucide/loader-circle";
  import RotateCcw from "~icons/lucide/rotate-ccw";

  let {
    documentLength = 0,
    lineCount = 1,
    line,
    col,
    selectionSize = 0,
    language = $bindable("auto"),
    detectedLanguage = "text",
    activeLanguage = "text",
    isCsvTableActive = false,
    csvInfo = { rows: 0, cols: 0, delimiter: "", errors: 0 },
    indentConfig = DEFAULT_INDENT_CONFIG,
    eol = "lf",
    encoding = "utf-8",
    canReopenEncoding = false,
    reopenEncodingDisabledReason = "Save this slate before reopening it.",
    onGoToLine = () => {},
    onOpenIndentPicker = () => {},
    onEolChange = () => {},
    onReopenEncoding = async () => false,
    onSaveEncoding = async () => false,
  }: {
    documentLength?: number;
    lineCount?: number;
    line: number;
    col: number;
    selectionSize?: number;
    language?: string;
    detectedLanguage?: string;
    activeLanguage?: string;
    isCsvTableActive?: boolean;
    csvInfo?: { rows: number; cols: number; delimiter: string; errors: number };
    indentConfig: IndentConfig;
    indentMode?: never;
    indentSize?: never;
    eol?: Eol;
    encoding?: CharacterEncoding;
    canReopenEncoding?: boolean;
    reopenEncodingDisabledReason?: string;
    onGoToLine?: () => void;
    onOpenIndentPicker?: () => void;
    onEolChange?: (next: Eol) => void;
    onReopenEncoding?: (next: CharacterEncoding) => Promise<boolean>;
    onSaveEncoding?: (next: CharacterEncoding) => Promise<boolean>;
  } = $props();

  const indentLabel = $derived.by(() => {
    switch (indentConfig.indentMode) {
      case IndentMode.Tab: return `Tab: ${indentConfig.indentSize}`;
      case IndentMode.Spaces:
      default: return `Spaces: ${indentConfig.indentSize}`;
    }
  });

  const showUpdateStatus = $derived(
    appMenuState.updatePolicy === "self-update" &&
      (appMenuState.updateStatus === "available" ||
        appMenuState.updateStatus === "installing" ||
        appMenuState.updateStatus === "installed"),
  );
  const updateStatusLabel = $derived.by(() => {
    if (appMenuState.updateStatus === "installing") return "Updating…";
    if (appMenuState.updateStatus === "installed") return "Restart to update";
    return `Update v${appMenuState.availableVersion} available`;
  });
</script>

<div
  class="flex h-6 w-full shrink-0 items-center px-3 text-xs bg-sidebar border-t border-border/40 text-muted-foreground select-none font-normal"
>
  {#if showUpdateStatus}
    <AppTooltip
      content={appMenuState.updateStatus === "installed"
        ? "Open update details. Restart Grayslate when convenient to use the update."
        : "Open update details"}
    >
      {#snippet trigger({ props })}
        <button
          {...props}
          type="button"
          data-testid="status-update"
          data-update-status={appMenuState.updateStatus}
          class="ui-state flex h-full shrink-0 items-center gap-1 px-2"
          aria-label={updateStatusLabel}
          disabled={appMenuState.updateStatus === "installing"}
          onclick={() => {
            void openAboutDialog();
          }}
        >
          {#if appMenuState.updateStatus === "installing"}
            <LoaderCircle class="size-4 animate-spin" />
          {:else if appMenuState.updateStatus === "installed"}
            <RotateCcw class="size-4" />
          {:else}
            <Download class="size-4" />
          {/if}
          <span class="hidden sm:inline">{updateStatusLabel}</span>
        </button>
      {/snippet}
    </AppTooltip>
  {/if}
  <div class="ml-auto flex items-center h-full">
    {#if isCsvTableActive}
      <div
        data-testid="status-csv-info"
        class="flex items-center gap-3 px-2 h-full cursor-default border-r border-border/40"
      >
        <span>{csvInfo.rows} rows × {csvInfo.cols} cols</span>
        <span
          >Delimiter: <strong class="font-semibold">{csvInfo.delimiter}</strong
          ></span
        >
        {#if csvInfo.errors > 0}
          <span class="text-[hsl(0,80%,60%)]">
            ⚠ {csvInfo.errors} parse error{csvInfo.errors > 1 ? "s" : ""}
          </span>
        {/if}
      </div>
    {:else}
      <div class="flex items-center gap-2 h-full px-2 cursor-default">
        <span
          data-testid="status-length"
          data-doc-length={documentLength}
          data-line-count={lineCount}>Length {documentLength}, Lines {lineCount}</span>
        <span class="text-muted-foreground">|</span>
        <AppTooltip content={formatShortcutTooltip("Go to line", "go-to-line", platformState.osType)}>
          {#snippet trigger({ props })}
            <button
              {...props}
              type="button"
              data-testid="status-goto-line"
              class="ui-state h-full px-1.5 cursor-pointer"
              onclick={() => onGoToLine()}
            >
              Ln {line}, Col {col}
            </button>
          {/snippet}
        </AppTooltip>
        {#if selectionSize > 0}
          <span>({selectionSize} selected)</span>
        {/if}
        <span class="text-muted-foreground">|</span>
        <AppTooltip content="Select indentation">
          {#snippet trigger({ props })}
            <button
              {...props}
              type="button"
              data-testid="status-indent"
              class="ui-state h-full px-1.5 cursor-pointer"
              onclick={() => onOpenIndentPicker()}
            >
              {indentLabel}
            </button>
          {/snippet}
        </AppTooltip>
      </div>
    {/if}
    {#if !isCsvTableActive}
      <span class="text-muted-foreground">|</span>
      <EolPicker {eol} onChange={onEolChange} />
      <span class="text-muted-foreground">|</span>
      <EncodingPicker
        {encoding}
        canReopen={canReopenEncoding}
        reopenDisabledReason={reopenEncodingDisabledReason}
        onReopen={onReopenEncoding}
        onSave={onSaveEncoding}
      />
    {/if}
    <span class="text-muted-foreground">|</span>
    <LanguagePicker bind:language {detectedLanguage} />
  </div>
</div>
