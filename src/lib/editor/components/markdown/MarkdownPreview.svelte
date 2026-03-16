<script lang="ts">
  import DOMPurify from "dompurify";
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
  import type {
    MarkdownPreviewWorkerRequest,
    MarkdownPreviewWorkerResponse,
  } from "$lib/editor/workers/markdownPreviewProtocol";

  let { content, editorView }: { content: string; editorView?: EditorView } =
    $props();

  const PURIFY_CONFIG = { ADD_ATTR: ["data-line"] };
  const MARKDOWN_RENDER_DEBOUNCE_MS = 120;
  const MARKDOWN_RENDER_ERROR_HTML = "<p>Error parsing markdown</p>";

  let previewEl = $state<HTMLElement | undefined>(undefined);
  let htmlPreview = $state("");

  let markdownRenderWorker: Worker | undefined;
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

  function disposeMarkdownRenderWorker(): void {
    if (markdownRenderWorker) {
      markdownRenderWorker.onmessage = null;
      markdownRenderWorker.onerror = null;
      markdownRenderWorker.onmessageerror = null;
      markdownRenderWorker.terminate();
      markdownRenderWorker = undefined;
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

  function ensureMarkdownRenderWorker(): Worker {
    if (!markdownRenderWorker) {
      markdownRenderWorker = new Worker(
        new URL("../../workers/markdownPreview.worker.ts", import.meta.url),
        { type: "module" },
      );

      markdownRenderWorker.onmessage = (
        event: MessageEvent<MarkdownPreviewWorkerResponse>,
      ) => {
        const message = event.data;
        if (message.requestId !== latestMarkdownRenderRequestId) {
          return;
        }

        if (message.type === "error") {
          handleMarkdownRenderFailure(
            "Markdown preview render failed:",
            message.error,
          );
          return;
        }

        try {
          htmlPreview = DOMPurify.sanitize(message.html, PURIFY_CONFIG);
        } catch (error) {
          handleMarkdownRenderFailure(
            "Markdown preview sanitization failed:",
            error,
          );
        }
      };

      markdownRenderWorker.onerror = (event) => {
        handleMarkdownRenderFailure(
          "Markdown preview worker crashed:",
          event.error ?? event.message,
        );
        disposeMarkdownRenderWorker();
      };

      markdownRenderWorker.onmessageerror = (event) => {
        handleMarkdownRenderFailure(
          "Markdown preview worker returned an unreadable message:",
          event,
        );
        disposeMarkdownRenderWorker();
      };
    }

    return markdownRenderWorker;
  }

  function postMarkdownRenderRequest(nextContent: string): void {
    nextMarkdownRenderRequestId += 1;
    latestMarkdownRenderRequestId = nextMarkdownRenderRequestId;
    hasPostedMarkdownRenderRequest = true;

    const request: MarkdownPreviewWorkerRequest = {
      type: "render",
      requestId: nextMarkdownRenderRequestId,
      content: nextContent,
    };
    ensureMarkdownRenderWorker().postMessage(request);
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
      postMarkdownRenderRequest(nextContent);
    }, delay);

    return () => {
      clearMarkdownRenderTimer();
    };
  });

  // Bidirectional scroll sync
  $effect(() => {
    const fontSize = editorState.fontSize;
    if (!previewEl || !editorView) return;

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
    disposeMarkdownRenderWorker();
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
