<script lang="ts">
  import { createScrollSync } from "./scrollSync";
  import MarkdownPreviewContextMenu from "./MarkdownPreviewContextMenu.svelte";
  import { hotkey, type HotkeyBinding } from "$lib/hotkeys";
  import type { EditorView } from "codemirror";
  import { onDestroy } from "svelte";
  import { editorState } from "$lib/state/editor.svelte";
  import {
    activateMarkdownPreview,
    copyMarkdownPreviewSelectionOrAll,
    getMarkdownPreviewSelectionText,
    registerMarkdownPreviewElement,
    selectAllMarkdownPreview,
    unregisterMarkdownPreviewElement,
  } from "./previewActions";
  import { invokeText, invoke } from "$lib/ipc";

  let { content, editorView }: { content: string; editorView?: EditorView } =
    $props();

  const MARKDOWN_RENDER_DEBOUNCE_MS = 120;
  const MARKDOWN_RENDER_ERROR_HTML = "<p>Error parsing markdown</p>";

  let previewEl = $state<HTMLElement | undefined>(undefined);
  let htmlPreview = $state("");

  let nextMarkdownRenderRequestId = 0;
  let latestMarkdownRenderRequestId = 0;
  let markdownRenderTimer: ReturnType<typeof setTimeout> | undefined;
  let hasPostedMarkdownRenderRequest = false;

  const previewHotkeys: HotkeyBinding[] = [
    {
      key: "Mod+A",
      callback: (event) => {
        event.preventDefault();
        selectAllMarkdownPreview();
      },
      options: { ignoreInputs: false },
    },
    {
      key: "Mod+C",
      callback: (event) => {
        event.preventDefault();
        void copyMarkdownPreviewSelectionOrAll();
      },
      options: { ignoreInputs: false },
    },
  ];

  function activatePreviewSurface() {
    activateMarkdownPreview();
    editorState.currentSelectionSize = getMarkdownPreviewSelectionText().length;
    // Clear any CodeMirror selection so the two panes don't show
    // simultaneous highlights side-by-side.
    if (editorView) {
      const sel = editorView.state.selection;
      if (!sel.main.empty) {
        editorView.dispatch({
          selection: { anchor: sel.main.head },
          userEvent: "select",
        });
      }
    }
  }

  function clearMarkdownRenderTimer(): void {
    if (markdownRenderTimer != null) {
      clearTimeout(markdownRenderTimer);
      markdownRenderTimer = undefined;
    }
  }

  function handleMarkdownRenderFailure(
    message: string,
    error?: unknown,
  ): void {
    htmlPreview = MARKDOWN_RENDER_ERROR_HTML;
    if (error != null) {
      console.error(message, error);
      return;
    }
    console.error(message);
  }

  async function postMarkdownRenderRequest(nextContent: string): Promise<void> {
    nextMarkdownRenderRequestId += 1;
    const requestId = nextMarkdownRenderRequestId;
    latestMarkdownRenderRequestId = requestId;
    hasPostedMarkdownRenderRequest = true;

    try {
      const html = await invokeText("render_markdown_preview", {
        content: nextContent,
        requestId,
      });

      // Stale response — a newer request has been sent since this one.
      if (requestId !== latestMarkdownRenderRequestId) return;

      htmlPreview = html;
    } catch (error) {
      // Stale cancellation — the backend cancelled a superseded render.
      if (requestId !== latestMarkdownRenderRequestId) return;

      const message =
        error instanceof Error ? error.message : String(error);
      // Backend returns "Cancelled" when a render was explicitly aborted.
      if (message === "Cancelled") return;

      handleMarkdownRenderFailure("Markdown preview render failed:", error);
    }
  }

  $effect(() => {
    const nextContent = content;
    clearMarkdownRenderTimer();

    if (!nextContent) {
      latestMarkdownRenderRequestId = 0;
      hasPostedMarkdownRenderRequest = false;
      htmlPreview = "";
      return;
    }

    const delay = hasPostedMarkdownRenderRequest
      ? MARKDOWN_RENDER_DEBOUNCE_MS
      : 0;

    markdownRenderTimer = setTimeout(() => {
      markdownRenderTimer = undefined;
      void postMarkdownRenderRequest(nextContent);
    }, delay);

    return () => {
      clearMarkdownRenderTimer();
    };
  });

  // Bidirectional scroll sync
  $effect(() => {
    const fontSize = editorState.fontSize;
    if (!previewEl || !editorView) return;

    // Reset preview scroll to top when recreating sync (file switch or
    // first mount) so both panes start aligned at the document start.
    previewEl.scrollTop = 0;

    // Wait until the next paint so the rendered preview DOM is measurable.
    let syncCleanup: (() => void) | undefined;
    const previewElement = previewEl;
    const view = editorView;
    const frameId = requestAnimationFrame(() => {
      void fontSize;
      syncCleanup = createScrollSync(view, previewElement);
    });

    return () => {
      cancelAnimationFrame(frameId);
      syncCleanup?.();
    };
  });

  $effect(() => {
    if (!previewEl) return;

    const previewElement = previewEl;
    registerMarkdownPreviewElement(previewElement);
    return () => unregisterMarkdownPreviewElement(previewElement);
  });

  $effect(() => {
    if (!previewEl) return;

    function syncPreviewSelectionSize() {
      if (editorState.activeSurface !== "markdown-preview") {
        return;
      }

      editorState.currentSelectionSize =
        getMarkdownPreviewSelectionText().length;
    }

    document.addEventListener("selectionchange", syncPreviewSelectionSize);
    return () => {
      document.removeEventListener("selectionchange", syncPreviewSelectionSize);
    };
  });

  onDestroy(() => {
    clearMarkdownRenderTimer();
    // Cancel any in-flight backend render for this window.
    invoke("cancel_markdown_preview").catch(() => {});
    if (previewEl) {
      unregisterMarkdownPreviewElement(previewEl);
    }
    if (editorState.activeSurface === "markdown-preview") {
      editorState.currentSelectionSize = 0;
    }
    if (editorState.activeSurface === "markdown-preview") {
      editorState.activeSurface = editorView ? "editor" : undefined;
    }
    previewEl = undefined;
    editorView = undefined;
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_static_element_interactions -->
<div
  bind:this={previewEl}
  use:hotkey={previewHotkeys}
  class="selectable flex-1 w-full min-w-0 bg-background overflow-y-auto overscroll-none p-8 prose prose-sm dark:prose-invert max-w-none prose-pre:bg-[#ffffff] prose-pre:text-[#212121] dark:prose-pre:bg-[#23262E] dark:prose-pre:text-[#D5CED9] prose-code:text-[#212121] dark:prose-code:text-[#D5CED9] prose-pre:border prose-pre:border-border prose-pre:shadow-sm"
  style={`font-size: ${editorState.fontSize}px;`}
  tabindex="0"
  role="document"
  onpointerdown={activatePreviewSurface}
  onfocusin={activatePreviewSurface}
>
  {@html htmlPreview}
</div>

<MarkdownPreviewContextMenu {previewEl} />
