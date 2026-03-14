<script lang="ts">
  import type { EditorView } from "codemirror";
  import { untrack } from "svelte";
  import {
    closeEditorPopup,
    editorState,
    registerEditorPopup,
    syncEditorPopupOpenState,
  } from "$lib/state/editor.svelte";
  import {
    editorFindNext,
    editorFindPrevious,
    editorReplaceNext,
    editorReplaceAll,
    editorSetSearchQuery,
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
  let openCount = $state(0);
  let hasResolvedSearch = $state(false);

  // Debounce constant — how long to wait after the last keystroke before
  // dispatching the CM search query and running the full-document stats scan.
  const SEARCH_DEBOUNCE_MS = 150;
  const SEARCH_TEXTAREA_MIN_HEIGHT_PX = 36;
  const SEARCH_TEXTAREA_DEFAULT_WIDTH_PX = 240;
  const searchTextareaClass =
    "border-input bg-background selection:bg-primary dark:bg-input/30 selection:text-primary-foreground ring-offset-background placeholder:text-muted-foreground resize rounded-md border px-3 py-2 text-base shadow-xs transition-[color,box-shadow] outline-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm focus-visible:border-ring focus-visible:ring-1 focus-visible:ring-ring overflow-auto min-h-[36px] max-h-[200px] min-w-[220px] max-w-[420px]";
  let searchDebounceTimer: ReturnType<typeof setTimeout> | undefined;
  let pendingSearchView: EditorView | undefined;
  let pendingFindText = "";
  let pendingReplaceText = "";

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
    node.style.height = `${SEARCH_TEXTAREA_MIN_HEIGHT_PX}px`; // reset to single row to get accurate scrollHeight
    const maxH = 200; // matches max-h-[200px]
    node.style.height = `${Math.min(node.scrollHeight + 2, maxH)}px`;
  }

  function clearPendingSearchTimer() {
    if (searchDebounceTimer) {
      clearTimeout(searchDebounceTimer);
      searchDebounceTimer = undefined;
    }
  }

  function applyPendingSearch() {
    const targetView = pendingSearchView;
    const nextFindText = pendingFindText;
    const nextReplaceText = pendingReplaceText;
    clearPendingSearchTimer();
    if (!targetView) return;
    editorSetSearchQuery(targetView, nextFindText, nextReplaceText, false);
  }

  function flushPendingSearch() {
    if (!searchDebounceTimer) return;
    applyPendingSearch();
  }

  function findPreviousNow() {
    flushPendingSearch();
    editorFindPrevious(view, false);
  }

  function findNextNow() {
    flushPendingSearch();
    editorFindNext(view, false);
  }

  function replaceNextNow() {
    flushPendingSearch();
    editorReplaceNext(view, false);
  }

  function replaceAllNow() {
    flushPendingSearch();
    editorReplaceAll(view, false);
  }

  // Sync global → local and focus when the panel opens or is reopened while already visible.
  // openCount is tracked only inside the if-block so incrementing it from the open handler
  // re-runs this effect even when fr.visible was already true.
  $effect(() => {
    if (fr.visible) {
      openCount; // re-run on every open/reopen, not just when visibility flips
      untrack(() => {
        findText = fr.findText;
        replaceText = fr.replaceText;
      });
      // rAF fires before the next paint; $effect already runs post-DOM-flush so
      // findInputRef is bound, but rAF ensures focus happens atomically with rendering.
      const frameId = requestAnimationFrame(() => {
        if (!findInputRef) return;
        autoResizeFindOnOpen(findInputRef);
        findInputRef.focus();
        findInputRef.select();
      });
      return () => cancelAnimationFrame(frameId);
    }
  });

  $effect(() => {
    if (!fr.visible || !findText.length) {
      hasResolvedSearch = false;
      return;
    }

    if (!fr.searching) {
      hasResolvedSearch = true;
    }
  });

  // Sync local → global and drive the CodeMirror search query reactively.
  // Only the CodeMirror query dispatch is debounced here. Match counting is
  // handled asynchronously by a dedicated worker from the editor view-update
  // path, which keeps typing responsive on large documents.
  $effect(() => {
    fr.findText = findText;
    fr.replaceText = replaceText;

    if (view && fr.visible) {
      pendingSearchView = view;
      pendingFindText = findText;
      pendingReplaceText = replaceText;

      clearPendingSearchTimer();

      // Empty search: clear immediately — no need to debounce clearing
      if (!findText) {
        fr.searching = false;
        applyPendingSearch();
        pendingSearchView = undefined;
        fr.searching = false;
        return;
      }

      // Non-empty search: keep showing the previous count until the next worker
      // result arrives, but mark the state as pending so action handlers can
      // flush immediately if the user navigates before the debounce fires.
      fr.searching = true;
      searchDebounceTimer = setTimeout(() => {
        applyPendingSearch();
      }, SEARCH_DEBOUNCE_MS);

      return () => clearPendingSearchTimer();
    } else {
      clearPendingSearchTimer();
      pendingSearchView = undefined;
      fr.searching = false;
    }
  });

  function hide() {
    clearPendingSearchTimer();
    pendingSearchView = undefined;
    fr.visible = false;
    fr.findText = "";
    findText = "";
    // Clear the CM search query so match highlights don't linger
    if (view) {
      editorSetSearchQuery(view, "", "", false);
    }
  }

  function close() {
    closeEditorPopup("find-replace");
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

  $effect(() => {
    syncEditorPopupOpenState("find-replace", fr.visible);
  });

  $effect(() => {
    return registerEditorPopup("find-replace", {
      open: (request) => {
        if (request.id !== "find-replace") return;
        fr.replaceMode = request.replaceMode;
        fr.visible = true;
        openCount++;
      },
      close: hide,
    });
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
    class="absolute top-4 right-8 z-50 flex w-fit max-w-[80vw] max-h-[80vh] flex-col gap-2 rounded-md border border-border bg-popover p-3 shadow-md"
    role="dialog"
    aria-label="Find and Replace"
  >
    <div class="flex flex-col gap-2 w-full h-full">
      <!-- Find Row -->
      <div class="flex items-start gap-2">
        <Button
          variant="ghost"
          size="icon"
          class="shrink-0"
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
                callback: findNextNow,
                options: { ignoreInputs: false },
              },
              {
                key: "Shift+Enter",
                callback: findPreviousNow,
                options: { ignoreInputs: false },
              },
              {
                key: "Escape",
                callback: close,
                options: { ignoreInputs: false },
              },
            ]}
            placeholder="Find"
            class={searchTextareaClass}
            style={`width: ${SEARCH_TEXTAREA_DEFAULT_WIDTH_PX}px`}
            spellcheck="false"
            wrap="off"
            rows="1"
          ></textarea>
          {@render resizeGrip()}
        </div>
        <div class="ml-1 flex items-center gap-1 self-stretch border-l pl-2">
          {#if findText.length > 0}
            <span
              class="text-sm text-muted-foreground pointer-events-none inline-flex shrink-0 items-center justify-center whitespace-nowrap px-1.5 min-w-[5.5rem]"
            >
              {#if fr.matchCount > 0}
                {#if fr.currentMatch === 0}
                  {fr.matchCount}+
                {:else}
                  {fr.currentMatch}/{fr.matchCount}
                {/if}
              {:else if hasResolvedSearch}
                No results
              {/if}
            </span>
          {/if}
          <Button
            variant="ghost"
            size="icon"
            onclick={findPreviousNow}
            title="Previous match ({formatForDisplay('Shift+Enter')})"
            disabled={!canNavigate}
          >
            <ArrowUp class="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            onclick={findNextNow}
            title="Next match ({formatForDisplay('Enter')})"
            disabled={!canNavigate}
          >
            <ArrowDown class="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            class="ml-1"
            onclick={close}
            title="Close ({formatForDisplay('Escape')})"
          >
            <X class="h-4 w-4" />
          </Button>
        </div>
      </div>

      <!-- Replace Row -->
      {#if fr.replaceMode}
        <div class="flex items-start gap-2 pl-11">
          <div class="relative flex">
            <textarea
              bind:this={replaceTextareaRef}
              bind:value={replaceText}
              use:hotkey={[
                {
                  key: "Enter",
                  callback: replaceNextNow,
                  options: { ignoreInputs: false },
                },
                {
                  key: "Escape",
                  callback: close,
                  options: { ignoreInputs: false },
                },
              ]}
              placeholder="Replace"
              class={searchTextareaClass}
              style={`width: ${SEARCH_TEXTAREA_DEFAULT_WIDTH_PX}px`}
              spellcheck="false"
              wrap="off"
              rows="1"
            ></textarea>
            {@render resizeGrip()}
          </div>
          <div class="ml-1 flex items-center gap-1 self-stretch border-l pl-2">
            <Button
              variant="ghost"
              size="icon"
              onclick={replaceNextNow}
              title="Replace currently selected match"
              disabled={!canReplace}
            >
              <CodIconReplace class="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              onclick={replaceAllNow}
              title="Replace All matches"
              disabled={!canReplaceAll}
            >
              <CodIconReplaceAll class="h-4 w-4" />
            </Button>
            <!-- Invisible placeholder to match the Find row's Close button width for perfect horizontal alignment -->
            <div class="ml-1 size-9 shrink-0"></div>
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}
