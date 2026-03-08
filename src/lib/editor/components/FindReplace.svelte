<script lang="ts">
  import { untrack } from "svelte";
  import { editorState } from "$lib/state/editor.svelte";
  import {
    editorFindNext,
    editorFindPrevious,
    editorReplaceNext,
    editorReplaceAll,
    editorSetSearchQuery,
    updateSearchStats,
  } from "$lib/editor/core/actions";
  import { Button } from "$lib/components/ui/button";
  import { hotkey, registerHotkey } from "$lib/hotkeys";
  import { formatForDisplay } from "@tanstack/hotkeys";
  import ArrowUp from "~icons/lucide/arrow-up";
  import ArrowDown from "~icons/lucide/arrow-down";
  import X from "~icons/lucide/x";
  import ChevronDown from "~icons/lucide/chevron-down";
  import ChevronRight from "~icons/lucide/chevron-right";
  import Scaling from "~icons/lucide/scaling";
  import CodIconReplace from "~icons/codicon/replace";
  import CodIconReplaceAll from "~icons/codicon/replace-all";

  let findText = $state("");
  let replaceText = $state("");

  // Convenience aliases — avoids repeating long chains everywhere
  const fr = $derived(editorState.findReplace);
  const view = $derived(editorState.activeView);

  // Derived flags used for button disabled states
  const canNavigate = $derived(fr.matchCount > 1);
  const canReplace = $derived(fr.matchCount > 0 && replaceText.length > 0);
  const canReplaceAll = $derived(fr.matchCount > 1 && replaceText.length > 0);

  let findInputRef: HTMLTextAreaElement | null = $state(null);
  let replaceTextareaRef: HTMLTextAreaElement | null = $state(null);

  // Sync textarea widths: when either is resized, set the other to match.
  // Unobserve the target before writing its width to prevent feedback-loop jitter.
  $effect(() => {
    const find = findInputRef;
    const replace = replaceTextareaRef;
    if (!find && !replace) return;

    let syncing = false;

    const observer = new ResizeObserver((entries) => {
      if (syncing) return;
      syncing = true;
      for (const entry of entries) {
        const target = entry.target as HTMLTextAreaElement;
        const other = target === find ? replace : find;
        if (!other) continue;
        const w = target.offsetWidth;
        if (other.offsetWidth !== w) {
          observer.unobserve(other);
          other.style.width = `${w}px`;
          requestAnimationFrame(() => {
            if (other.isConnected) observer.observe(other);
            syncing = false;
          });
          return; // one sync per frame is enough
        }
      }
      syncing = false;
    });

    if (find) observer.observe(find);
    if (replace) observer.observe(replace);
    return () => observer.disconnect();
  });

  // Auto-size the find textarea on open based on how much text is in it
  function autoResizeFindOnOpen(node: HTMLTextAreaElement | null) {
    if (!node) return;
    node.style.height = "30px"; // reset to single row to get accurate scrollHeight
    const maxH = 200; // matches max-h-[200px]
    node.style.height = `${Math.min(node.scrollHeight + 2, maxH)}px`;
  }

  // Sync global → local when panel first becomes visible
  $effect(() => {
    if (fr.visible) {
      untrack(() => {
        findText = fr.findText;
        replaceText = fr.replaceText;
      });

      // Focus and select the existing text inside the find box
      if (findInputRef) {
        // We use setTimeout so it happens after the DOM has fully rendered
        setTimeout(() => {
          autoResizeFindOnOpen(findInputRef);
          findInputRef?.focus();
        }, 10);
      }
    }
  });

  // Sync local → global and drive the CodeMirror search query reactively
  $effect(() => {
    fr.findText = findText;
    fr.replaceText = replaceText;

    if (view && fr.visible) {
      editorSetSearchQuery(view, findText, replaceText, false);
      updateSearchStats(view, { forceRescan: true });
    }
  });

  function close() {
    fr.visible = false;
    view?.focus();
  }

  function toggleReplaceMode() {
    fr.replaceMode = !fr.replaceMode;
  }

  $effect(() => {
    if (!fr.visible) return;
    return registerHotkey(
      "Escape",
      () => {
        close();
      },
      { ignoreInputs: false },
    );
  });
</script>

{#snippet resizeGrip()}
  <Scaling
    class="absolute bottom-1 right-1 h-3.5 w-3.5 pointer-events-none text-muted-foreground rotate-90 cursor-nwse-resize"
  />
{/snippet}

{#if editorState.findReplace.visible}
  <!-- Floating Find & Replace Panel -->
  <div
    class="absolute top-4 right-8 z-50 flex flex-col gap-2 rounded-md border border-border bg-popover p-2 shadow-md w-fit max-w-[80vw] max-h-[80vh]"
    role="dialog"
    aria-label="Find and Replace"
  >
    <div class="flex flex-col gap-2 w-full h-full">
      <!-- Find Row -->
      <div class="flex items-start gap-1">
        <Button
          variant="ghost"
          size="icon"
          class="h-6 w-6 shrink-0"
          onclick={toggleReplaceMode}
          title="Toggle Replace"
        >
          {#if fr.replaceMode}
            <ChevronDown class="h-4 w-4" />
          {:else}
            <ChevronRight class="h-4 w-4" />
          {/if}
        </Button>
        <div class="relative flex">
          <textarea
            bind:this={findInputRef}
            bind:value={findText}
            use:hotkey={[
              {
                key: "Enter",
                callback: () => editorFindNext(view, false),
                options: { ignoreInputs: false },
              },
              {
                key: "Shift+Enter",
                callback: () => editorFindPrevious(view, false),
                options: { ignoreInputs: false },
              },
              {
                key: "Escape",
                callback: close,
                options: { ignoreInputs: false },
              },
            ]}
            placeholder="Find"
            class="min-h-[30px] max-h-[200px] min-w-[160px] max-w-[400px] resize text-xs placeholder:text-muted-foreground/50 border border-input focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring bg-transparent rounded-md py-1.5 px-2 overflow-auto"
            style="width: 220px"
            spellcheck="false"
            wrap="off"
            rows="1"
          ></textarea>
          {@render resizeGrip()}
        </div>
        <div class="flex items-center border-l pl-1 ml-1 self-stretch gap-0.5">
          {#if findText.length > 0}
            <span
              class="text-xs text-muted-foreground pointer-events-none whitespace-nowrap px-1 shrink-0"
            >
              {#if fr.matchCount > 0}
                {#if fr.currentMatch === 0}
                  {fr.matchCount}+
                {:else}
                  {fr.currentMatch}/{fr.matchCount}
                {/if}
              {:else}
                No results
              {/if}
            </span>
          {/if}
          <Button
            variant="ghost"
            size="icon"
            class="h-6 w-6"
            onclick={() => editorFindPrevious(view, false)}
            title="Previous match ({formatForDisplay('Shift+Enter')})"
            disabled={!canNavigate}
          >
            <ArrowUp class="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            class="h-6 w-6"
            onclick={() => editorFindNext(view, false)}
            title="Next match ({formatForDisplay('Enter')})"
            disabled={!canNavigate}
          >
            <ArrowDown class="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            class="h-6 w-6 ml-1"
            onclick={close}
            title="Close ({formatForDisplay('Escape')})"
          >
            <X class="h-4 w-4" />
          </Button>
        </div>
      </div>

      <!-- Replace Row -->
      {#if fr.replaceMode}
        <div class="flex items-start gap-1 pl-7">
          <div class="relative flex">
            <textarea
              bind:this={replaceTextareaRef}
              bind:value={replaceText}
              use:hotkey={[
                {
                  key: "Enter",
                  callback: () => editorReplaceNext(view, false),
                  options: { ignoreInputs: false },
                },
                {
                  key: "Escape",
                  callback: close,
                  options: { ignoreInputs: false },
                },
              ]}
              placeholder="Replace"
              class="min-h-[30px] max-h-[200px] min-w-[160px] max-w-[400px] resize text-xs placeholder:text-muted-foreground/50 border border-input focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring bg-transparent rounded-md px-2 py-1.5 overflow-auto"
              style="width: 220px"
              spellcheck="false"
              wrap="off"
              rows="1"
            ></textarea>
            {@render resizeGrip()}
          </div>
          <div
            class="flex items-start border-l pl-1 ml-1 gap-1 pt-1 self-stretch"
          >
            <Button
              variant="ghost"
              size="icon"
              class="h-6 w-6"
              onclick={() => editorReplaceNext(view, false)}
              title="Replace currently selected match"
              disabled={!canReplace}
            >
              <CodIconReplace class="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              class="h-6 w-6"
              onclick={() => editorReplaceAll(view, false)}
              title="Replace All matches"
              disabled={!canReplaceAll}
            >
              <CodIconReplaceAll class="h-4 w-4" />
            </Button>
            <!-- Invisible placeholder to match the Find row's Close button width for perfect horizontal alignment -->
            <div class="h-6 w-6 ml-1 shrink-0"></div>
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}
