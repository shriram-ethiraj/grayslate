<script lang="ts">
  import LanguagePicker from "./LanguagePicker.svelte";

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
    onGoToLine = () => {},
  } = $props();
</script>

<div
  class="flex h-6 w-full shrink-0 items-center justify-end px-3 text-xs bg-sidebar border-t border-border/40 text-muted-foreground select-none font-medium"
>
  <div class="flex items-center h-full">
    {#if isCsvTableActive}
      <div
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
        <span>Length {documentLength}, Lines {lineCount}</span>
        <span class="text-border/80">|</span>
        <button
          type="button"
          title="Go to line"
          class="hover:bg-muted/50 hover:text-foreground h-full px-1.5 transition-colors cursor-pointer"
          onclick={() => onGoToLine()}
        >
          Ln {line}, Col {col}
        </button>
        {#if selectionSize > 0}
          <span>({selectionSize} selected)</span>
        {/if}
      </div>
    {/if}
    <span class="text-border/80">|</span>
    <LanguagePicker bind:language {detectedLanguage} />
  </div>
</div>
