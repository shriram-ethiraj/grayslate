<script lang="ts">
  import { createScrollSync } from "./scrollSync";
  import MarkdownPreviewContextMenu from "./MarkdownPreviewContextMenu.svelte";
  import { hotkey, type HotkeyBinding } from "$lib/hotkeys";
  import type { EditorView } from "codemirror";
  import { onDestroy, tick } from "svelte";
  import { toast } from "$lib/components/ui/sonner";
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
  import { prepareMarkdownPreviewHtml } from "./previewHtml";

  interface Props {
    content: string;
    editorView?: EditorView;
    documentId?: string;
    documentGeneration?: number;
  }

  let { content, editorView, documentId, documentGeneration }: Props = $props();

  const MARKDOWN_RENDER_DEBOUNCE_MS = 120;
  const MAX_MARKDOWN_PREVIEW_BYTES = 5 * 1024 * 1024;
  let previewEl = $state<HTMLElement | undefined>(undefined);
  let htmlPreview = $state("");
  let previewNotice = $state<string | undefined>(undefined);

  let nextMarkdownRenderRequestId = 0;
  let latestMarkdownRenderRequestId = 0;
  let markdownRenderTimer: ReturnType<typeof setTimeout> | undefined;
  let hasPostedMarkdownRenderRequest = false;
  let activeObjectUrls: string[] = [];
  let renderedAuthorization:
    | { documentId: string; documentGeneration: number }
    | undefined;

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
    htmlPreview = "";
    previewNotice = "Markdown preview could not be rendered.";
    if (error != null) {
      console.error(message, error);
      return;
    }
    console.error(message);
  }

  function revokeObjectUrls(urls: string[]): void {
    for (const url of urls) URL.revokeObjectURL(url);
  }

  async function replacePreviewHtml(
    html: string,
    objectUrls: string[],
    nextDocumentId: string | undefined,
    nextDocumentGeneration: number | undefined,
  ): Promise<void> {
    const previousObjectUrls = activeObjectUrls;
    activeObjectUrls = objectUrls;
    renderedAuthorization =
      nextDocumentId && nextDocumentGeneration !== undefined
        ? {
            documentId: nextDocumentId,
            documentGeneration: nextDocumentGeneration,
          }
        : undefined;
    previewNotice = undefined;
    htmlPreview = html;
    await tick();
    revokeObjectUrls(previousObjectUrls);
  }

  function exceedsPreviewLimit(value: string): boolean {
    if (value.length > MAX_MARKDOWN_PREVIEW_BYTES) return true;
    if (value.length <= Math.floor(MAX_MARKDOWN_PREVIEW_BYTES / 3)) return false;
    return new TextEncoder().encode(value).byteLength > MAX_MARKDOWN_PREVIEW_BYTES;
  }

  function cancelActiveRender(): void {
    latestMarkdownRenderRequestId = ++nextMarkdownRenderRequestId;
    void invoke("cancel_markdown_preview").catch(() => {});
  }

  async function postMarkdownRenderRequest(
    nextContent: string,
    nextDocumentId: string | undefined,
    nextDocumentGeneration: number | undefined,
  ): Promise<void> {
    nextMarkdownRenderRequestId += 1;
    const requestId = nextMarkdownRenderRequestId;
    latestMarkdownRenderRequestId = requestId;
    hasPostedMarkdownRenderRequest = true;

    try {
      const html = await invokeText("render_markdown_preview", {
        content: nextContent,
      });

      // Stale response — a newer request has been sent since this one.
      if (requestId !== latestMarkdownRenderRequestId) return;

      const prepared = await prepareMarkdownPreviewHtml(
        html,
        nextDocumentId && nextDocumentGeneration !== undefined
          ? { documentId: nextDocumentId, documentGeneration: nextDocumentGeneration }
          : undefined,
      );
      if (requestId !== latestMarkdownRenderRequestId) {
        revokeObjectUrls(prepared.objectUrls);
        return;
      }

      await replacePreviewHtml(
        prepared.html,
        prepared.objectUrls,
        nextDocumentId,
        nextDocumentGeneration,
      );
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
    const nextDocumentId = documentId;
    const nextDocumentGeneration = documentGeneration;
    clearMarkdownRenderTimer();

    if (!nextContent) {
      cancelActiveRender();
      hasPostedMarkdownRenderRequest = false;
      htmlPreview = "";
      previewNotice = undefined;
      renderedAuthorization = undefined;
      revokeObjectUrls(activeObjectUrls);
      activeObjectUrls = [];
      return;
    }

    if (exceedsPreviewLimit(nextContent)) {
      cancelActiveRender();
      hasPostedMarkdownRenderRequest = false;
      htmlPreview = "";
      previewNotice = "Markdown preview is available for documents up to 5 MB.";
      renderedAuthorization = undefined;
      revokeObjectUrls(activeObjectUrls);
      activeObjectUrls = [];
      return;
    }

    const delay = hasPostedMarkdownRenderRequest
      ? MARKDOWN_RENDER_DEBOUNCE_MS
      : 0;

    markdownRenderTimer = setTimeout(() => {
      markdownRenderTimer = undefined;
      void postMarkdownRenderRequest(
        nextContent,
        nextDocumentId,
        nextDocumentGeneration,
      );
    }, delay);

    return () => {
      clearMarkdownRenderTimer();
    };
  });

  function scrollToPreviewFragment(href: string): boolean {
    if (!previewEl || !href.startsWith("#")) return false;

    let fragment: string;
    try {
      fragment = decodeURIComponent(href.slice(1));
    } catch {
      return false;
    }

    const target = Array.from(previewEl.querySelectorAll<HTMLElement>("[id]")).find(
      (element) => element.id === fragment,
    );
    if (!target) return false;
    target.scrollIntoView({ block: "start" });
    return true;
  }

  async function handlePreviewClick(event: MouseEvent): Promise<void> {
    if ((event.button !== 0 && event.button !== 1) || !(event.target instanceof Element)) return;

    const anchor = event.target.closest<HTMLAnchorElement>("a[href]");
    if (!anchor || !previewEl?.contains(anchor)) return;

    const href = anchor.getAttribute("href")?.trim();
    if (!href) return;

    event.preventDefault();

    if (href.startsWith("#")) {
      if (!scrollToPreviewFragment(href)) {
        toast.error("The linked section was not found");
      }
      return;
    }

    try {
      await invoke("open_markdown_link", {
        href,
        documentId: renderedAuthorization?.documentId,
        documentGeneration: renderedAuthorization?.documentGeneration,
      });
    } catch {
      toast.error("Failed to open the Markdown link");
    }
  }

  $effect(() => {
    if (!previewEl) return;

    const previewElement = previewEl;
    function onPreviewLinkClick(event: MouseEvent): void {
      void handlePreviewClick(event);
    }

    previewElement.addEventListener("click", onPreviewLinkClick);
    previewElement.addEventListener("auxclick", onPreviewLinkClick);
    return () => {
      previewElement.removeEventListener("click", onPreviewLinkClick);
      previewElement.removeEventListener("auxclick", onPreviewLinkClick);
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
    latestMarkdownRenderRequestId = ++nextMarkdownRenderRequestId;
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
    revokeObjectUrls(activeObjectUrls);
    activeObjectUrls = [];
    htmlPreview = "";
    previewNotice = undefined;
    renderedAuthorization = undefined;
    previewEl = undefined;
    editorView = undefined;
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_static_element_interactions -->
<div
  bind:this={previewEl}
  data-testid="markdown-preview"
  use:hotkey={previewHotkeys}
  class="markdown-preview selectable flex-1 min-h-0 w-full min-w-0 bg-background overflow-y-auto overscroll-none p-8 prose prose-sm dark:prose-invert max-w-none prose-pre:bg-[#ffffff] prose-pre:text-[#212121] dark:prose-pre:bg-[#23262E] dark:prose-pre:text-[#D5CED9] prose-code:text-[#212121] dark:prose-code:text-[#D5CED9] prose-pre:border prose-pre:border-border prose-pre:shadow-sm"
  style={`font-size: ${editorState.fontSize}px;`}
  tabindex="0"
  role="document"
  onpointerdown={activatePreviewSurface}
  onfocusin={activatePreviewSurface}
>
  {#if previewNotice}
    <div class="not-prose p-6 py-16 text-center text-sm text-muted-foreground">
      {previewNotice}
    </div>
  {:else}
    {@html htmlPreview}
  {/if}
</div>

<MarkdownPreviewContextMenu {previewEl} />

<style>
  :global(.markdown-preview h1),
  :global(.markdown-preview h2) {
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.3em;
  }

  :global(.markdown-preview [align="center"]) {
    text-align: center;
  }

  :global(.markdown-preview [align="center"] > p) {
    text-align: center;
  }

  :global(.markdown-preview [align="right"]) {
    text-align: right;
  }

  :global(.markdown-preview [align="center"] > img),
  :global(.markdown-preview [align="center"] picture > img) {
    margin-inline: auto;
  }

  :global(.markdown-preview p > a > img) {
    display: inline-block;
    margin: 0;
    vertical-align: middle;
  }

  :global(.markdown-preview li:has(> input[type="checkbox"])) {
    list-style-type: none;
  }

  :global(.markdown-preview li > input[type="checkbox"]) {
    margin-inline: 0 0.5em;
    vertical-align: middle;
  }

  :global(.markdown-preview th[align="center"]),
  :global(.markdown-preview td[align="center"]) {
    text-align: center;
  }

  :global(.markdown-preview th[align="right"]),
  :global(.markdown-preview td[align="right"]) {
    text-align: right;
  }

  :global(.markdown-preview th[align="left"]),
  :global(.markdown-preview td[align="left"]) {
    text-align: left;
  }

  :global(.markdown-preview .markdown-table-scroll) {
    margin-block: 2em;
    overflow-x: auto;
  }

  :global(.markdown-preview .markdown-table-scroll > table) {
    margin-block: 0;
  }
</style>
