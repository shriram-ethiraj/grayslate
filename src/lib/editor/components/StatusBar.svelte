<script lang="ts">
  import LanguagePicker from "./LanguagePicker.svelte";
  import { IndentMode, type IndentConfig } from "./IndentationPicker.svelte";
  import { DEFAULT_INDENT_CONFIG } from "$lib/editor/core/editorSession";

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
    onGoToLine = () => {},
    onOpenIndentPicker = () => {},
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
    onGoToLine?: () => void;
    onOpenIndentPicker?: () => void;
  } = $props();

  const indentLabel = $derived.by(() => {
    switch (indentConfig.indentMode) {
      case IndentMode.Tab: return `Tab: ${indentConfig.indentSize}`;
      case IndentMode.Spaces:
      default: return `Spaces: ${indentConfig.indentSize}`;
    }
  });
</script>

<div
  class="flex h-6 w-full shrink-0 items-center justify-end px-3 text-xs bg-sidebar border-t border-border/40 text-muted-foreground select-none font-medium"
>
  <div class="flex items-center h-full">
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
        <button
          type="button"
          data-testid="status-goto-line"
          title="Go to line"
          class="hover:bg-muted/50 hover:text-foreground h-full px-1.5 transition-colors cursor-pointer"
          onclick={() => onGoToLine()}
        >
          Ln {line}, Col {col}
        </button>
        {#if selectionSize > 0}
          <span>({selectionSize} selected)</span>
        {/if}
        <span class="text-muted-foreground">|</span>
        <button
          type="button"
          data-testid="status-indent"
          title="Select Indentation"
          class="hover:bg-muted/50 hover:text-foreground h-full px-1.5 transition-colors cursor-pointer"
          onclick={() => onOpenIndentPicker()}
        >
          {indentLabel}
        </button>
      </div>
    {/if}
    <span class="text-muted-foreground">|</span>
    <LanguagePicker bind:language {detectedLanguage} />
  </div>
</div>
