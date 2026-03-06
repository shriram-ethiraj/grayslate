<script lang="ts">
  import Editor from "$lib/editor/components/Editor.svelte";
  import MarkdownPreview from "$lib/editor/components/markdown/MarkdownPreview.svelte";
  import CsvTableView from "./csv/CsvTableView.svelte";
  import StatusBar from "$lib/editor/components/StatusBar.svelte";
  import EditorLoader from "$lib/editor/components/EditorLoader.svelte";
  import {
    ResizablePaneGroup,
    ResizablePane,
    ResizableHandle,
  } from "$lib/components/ui/resizable";
  import { languageDetector } from "$lib/editor/core/languageDetector";
  import { debounce } from "lodash-es";
  import type { EditorView } from "codemirror";
  import {
    createManagedEditorSession,
    dispatchManagedEditorTextChange,
    type ManagedEditorSession,
  } from "$lib/editor/core/editorSession";
  import {
    editorState,
    updateEditorLoader,
    hideEditorLoader,
    startLoaderTicker,
    stopLoaderTicker,
    type FileType,
  } from "$lib/state/editor.svelte";
  import { open as openFilePicker } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { reclaimMemory } from "$lib/editor/core/memory";

  let value = $state("");
  let line = $state(1);
  let col = $state(1);
  let selectionSize = $state(0);
  let language = $state("auto");
  let detectedLanguage = $state("text");

  // Compute the actual language to apply to the editor
  let activeLanguage = $derived(
    language === "auto" ? detectedLanguage : language,
  );

  let activeFilePath = $state("");

  // Sync activeLanguage to global editorState
  $effect(() => {
    editorState.fileType = activeLanguage as FileType;
  });

  const checkLanguage = debounce((content: string) => {
    if (language === "auto" && content) {
      const result = languageDetector.detect(content);
      if (result) {
        detectedLanguage = result;
      }
    }
  }, 1000);

  // Watch the `value` and run language detection when it changes
  $effect(() => {
    checkLanguage(value);
  });

  let editorView = $state<EditorView | undefined>(undefined);
  let editorSession = $state.raw<ManagedEditorSession>(
    createManagedEditorSession(),
  );

  // -----------------------------------------------------------------------
  // Menu: "File > Open File..."
  //
  // The Tauri menu emits "menu://open-file" when the user clicks the item
  // (or presses Ctrl/Cmd+O via its accelerator). We open a native file
  // picker, then invoke read_file_content on the Rust side which enforces
  // the 50 MB size limit before returning the text.
  // -----------------------------------------------------------------------
  async function openFile(): Promise<void> {
    const selected = await openFilePicker({
      multiple: false,
      directory: false,
    });

    // User cancelled the dialog
    if (!selected) return;

    const filePath = selected as string;
    const filename = filePath.replace(/\\/g, "/").split("/").pop() ?? "";

    // Start a decelerating progress ticker while the Rust read is in-flight
    startLoaderTicker("Reading file…", filename, {
      ceiling: 65,
      factor: 0.06,
      minStep: 0.3,
      interval: 80,
      startAt: 5,
    });

    try {
      const content = await invoke<string>("read_file_content", {
        path: filePath,
      });

      stopLoaderTicker();
      updateEditorLoader("Detecting language…", filename, 72);

      // Yield to let the UI repaint before running language detection
      await new Promise<void>((r) => setTimeout(r, 0));
      const detected = languageDetector.detect(content, filename) ?? "text";

      updateEditorLoader("Loading into editor…", filename, 88);
      await new Promise<void>((r) => setTimeout(r, 0));

      // If the filename's extension alone resolves to a language,
      // pin it directly — no need for "auto" mode, and the debounced
      const extLang = languageDetector.detect("", filename);
      if (extLang) {
        language = extLang;
        detectedLanguage = extLang;
      } else {
        language = "auto";
        detectedLanguage = detected;
      }

      activeFilePath = filePath;
  editorSession = createManagedEditorSession();
      value = content;

      // Yield to let Svelte update the DOM and dispose old CodeMirror instance
      await new Promise<void>((r) => setTimeout(r, 10));

      // Reclaim stale heap from the previous file (full-page-commit strategy)
      reclaimMemory();
    } catch (err: unknown) {
      const msg = typeof err === "string" ? err : "Failed to open file.";
      toast.error(msg);
    } finally {
      // Always clean up — idempotent in the success path
      stopLoaderTicker();
      hideEditorLoader();
    }
  }

  // Register (and later clean up) the menu-event listener.
  $effect(() => {
    // load the event helper only when this effect runs so the static
    // import can be removed. the promise resolves to an `unlisten` function
    // which we call on cleanup.
    const unlistenPromise = import("@tauri-apps/api/event").then(({ listen }) =>
      listen("menu://open-file", () => openFile()),
    );

    return () => {
      unlistenPromise.then((fn) => fn());
    };
  });

  // Derive whether CSV table view is active
  let isCsvTableActive = $derived(
    activeLanguage === "csv" && editorState.csv.showTable,
  );

  let csvInfo = $state({ rows: 0, cols: 0, delimiter: "", errors: 0 });
</script>

<div class="flex flex-1 flex-col min-h-0 min-w-0">
  <div class="flex flex-1 min-h-0 min-w-0 relative">
    <EditorLoader
      visible={editorState.loader.visible}
      message={editorState.loader.message}
      subMessage={editorState.loader.subMessage}
      progress={editorState.loader.progress}
    />

    {#if activeLanguage === "csv"}
      {#if isCsvTableActive}
        <div class="flex flex-1 flex-col min-h-0 min-w-0">
          <CsvTableView
            bind:content={value}
            bind:tableInfo={csvInfo}
            onMirrorTextChange={(nextText: string, userEvent: string) => {
              value = nextText;
              dispatchManagedEditorTextChange(editorSession, nextText, {
                userEvent,
                focus: false,
              });
            }}
          />
        </div>
      {:else}
        <div class="relative flex-1 min-h-0 min-w-0">
          <div class="absolute inset-0">
            {#key activeFilePath}
              <Editor
                bind:value
                bind:line
                bind:col
                bind:selectionSize
                language={activeLanguage}
                bind:editorView
                session={editorSession}
              />
            {/key}
          </div>
        </div>
      {/if}
    {:else if activeLanguage === "markdown"}
      <!--
                Markdown mode: ResizablePaneGroup keeps the Editor (pane 1)
                permanently mounted. Only pane 2 (preview) is conditionally
                appended so the editor never reloads when toggling the preview.
            -->
      <ResizablePaneGroup direction="horizontal" class="flex-1 min-h-0">
        <ResizablePane defaultSize={50} minSize={15} class="relative min-h-0">
          <div class="absolute inset-0">
            {#key activeFilePath}
              <Editor
                bind:value
                bind:line
                bind:col
                bind:selectionSize
                language={activeLanguage}
                bind:editorView
                session={editorSession}
              />
            {/key}
          </div>
        </ResizablePane>

        {#if editorState.markdown.showPreview}
          <ResizableHandle />
          <ResizablePane
            defaultSize={50}
            minSize={15}
            class="flex flex-col min-h-0"
          >
            <MarkdownPreview content={value} {editorView} />
          </ResizablePane>
        {/if}
      </ResizablePaneGroup>
    {:else}
      <!--
                All other modes: plain editor, no pane group overhead.
                absolute inset-0 inside relative flex-1 min-h-0 is the
                standard sizing pattern used throughout this app.
            -->
      <div class="relative flex-1 min-h-0 min-w-0">
        <div class="absolute inset-0">
          {#key activeFilePath}
            <Editor
              bind:value
              bind:line
              bind:col
              bind:selectionSize
              language={activeLanguage}
              bind:editorView
              session={editorSession}
            />
          {/key}
        </div>
      </div>
    {/if}
  </div>
  <StatusBar
    {line}
    {col}
    {selectionSize}
    bind:language
    {detectedLanguage}
    {activeLanguage}
    {isCsvTableActive}
    {csvInfo}
  />
</div>
