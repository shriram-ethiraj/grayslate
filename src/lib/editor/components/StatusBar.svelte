<script lang="ts">
  import type { Component } from "svelte";
  import Bug from "~icons/lucide/bug";
  import LanguagePicker from "./LanguagePicker.svelte";
  import {
    gcDebugControls,
    setGcDebugEnabled,
    toggleGcDebugPanel,
  } from "$lib/state/gc-debug-controls.svelte";

  let {
    line,
    col,
    selectionSize = 0,
    language = $bindable("auto"),
    detectedLanguage = "text",
    activeLanguage = "text",
    isCsvTableActive = false,
    csvInfo = { rows: 0, cols: 0, delimiter: "", errors: 0 },
  } = $props();

  let GcDebugDialogComponent = $state<Component | null>(null);

  $effect(() => {
    if (!gcDebugControls.enabled) {
      GcDebugDialogComponent = null;
      return;
    }

    if (GcDebugDialogComponent) {
      return;
    }

    let cancelled = false;

    void import("$lib/editor/components/GcDebugDialog.svelte").then((mod) => {
      if (!cancelled) {
        GcDebugDialogComponent = mod.default;
      }
    });

    return () => {
      cancelled = true;
    };
  });
</script>

<div
  class="flex h-6 w-full shrink-0 items-center justify-between px-3 text-[11px] bg-sidebar border-t border-border/40 text-muted-foreground select-none font-medium"
>
  <div class="flex items-center space-x-3 h-full">
    <button
      class="flex h-full items-center rounded px-2 {gcDebugControls.enabled
        ? 'text-sky-500 hover:text-sky-400'
        : 'text-foreground/80 hover:bg-muted/50 hover:text-foreground'}"
      onclick={() => {
        if (!gcDebugControls.enabled) {
          setGcDebugEnabled(true);
        }
        toggleGcDebugPanel();
      }}
      aria-label="Toggle GC diagnostics"
      title="Toggle GC diagnostics"
    >
      <Bug class="h-3.5 w-3.5" />
    </button>
  </div>
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
      <button
        class="flex items-center hover:bg-muted/50 hover:text-foreground h-full px-2 transition-colors cursor-default"
      >
        Ln {line}, Col {col}{#if selectionSize > 0}&nbsp;({selectionSize}
          selected){/if}
      </button>
    {/if}
    <LanguagePicker bind:language {detectedLanguage} />
  </div>
</div>

{#if gcDebugControls.enabled && GcDebugDialogComponent}
  <GcDebugDialogComponent />
{/if}
