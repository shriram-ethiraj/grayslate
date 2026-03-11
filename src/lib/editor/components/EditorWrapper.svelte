<script lang="ts">
  import Editor from "$lib/editor/components/Editor.svelte";
  import MarkdownPreview from "$lib/editor/components/markdown/MarkdownPreview.svelte";
  import CsvTableView from "./csv/CsvTableView.svelte";
  import StatusBar from "$lib/editor/components/StatusBar.svelte";
  import EditorLoader from "$lib/editor/components/EditorLoader.svelte";
  import GoToLineDialog from "$lib/editor/components/GoToLineDialog.svelte";
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
    disposeManagedEditorSession,
    ensureManagedEditorState,
    flushPendingValueSync,
    type ManagedEditorSession,
  } from "$lib/editor/core/editorSession";
  import {
    type CsvMirrorTextUpdate,
    type CsvTableFlushResult,
  } from "./csv/csvTableProtocol";
  import {
    editorState,
    updateEditorLoader,
    hideEditorLoader,
    startLoaderTicker,
    stopLoaderTicker,
    completeEditorLoader,
    type FileType,
  } from "$lib/state/editor.svelte";
  import { resolveNotesRoot } from "$lib/files/notesRoot";
  import {
    basename,
    join,
  } from "@tauri-apps/api/path";
  import {
    open as openFilePicker,
    save as saveFilePicker,
  } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { requestFileOpenReclaim } from "$lib/editor/core/memory";
  import { clearSearchStatsCache } from "$lib/editor/core/actions";
  import { clearColorCache } from "$lib/editor/extensions/colorHints";
  import { registerHotkey } from "$lib/hotkeys";
  import {
    librarySidebarState,
    clearPendingSidebarOpenFile,
    setPendingSidebarOpenFile,
  } from "$lib/state/librarySidebar.svelte";
  import {
    OPEN_FILE_PATH_EVENT,
    prepareFileOpen,
    RECENT_FILES_UPDATED_EVENT,
    type OpenFilePathPayload,
  } from "$lib/files/recentFiles";

  type SavedDocumentSource = "file";

  type ActiveDocument =
    | {
        kind: "untitled";
        key: string;
        createdAt: number;
        lastSavedValue: string;
      }
    | {
        kind: "saved";
        path: string;
        source: SavedDocumentSource;
        lastSavedValue: string;
      };

  let value = $state("");
  let documentLength = $state(0);
  let lineCount = $state(1);
  let line = $state(1);
  let col = $state(1);
  let selectionSize = $state(0);
  let language = $state("auto");
  let detectedLanguage = $state("text");
  let goToLineOpen = $state(false);

  function countDocumentLines(text: string): number {
    if (text.length === 0) {
      return 1;
    }

    let count = 1;

    for (let index = 0; index < text.length; index += 1) {
      if (text.charCodeAt(index) === 10) {
        count += 1;
      }
    }

    return count;
  }

  function createUntitledDocument(now = Date.now()): ActiveDocument {
    return {
      kind: "untitled",
      key: `untitled:${now}`,
      createdAt: now,
      lastSavedValue: "",
    };
  }

  function getDocumentKey(document: ActiveDocument): string {
    return document.kind === "untitled" ? document.key : document.path;
  }

  function getPreferredExtension(fileType: string): string {
    switch (fileType) {
      case "csv":
        return "csv";
      case "markdown":
        return "md";
      case "json":
        return "json";
      case "javascript":
        return "js";
      case "typescript":
        return "ts";
      case "python":
        return "py";
      case "html":
        return "html";
      case "css":
        return "css";
      case "yaml":
        return "yaml";
      case "c":
        return "c";
      case "cpp":
        return "cpp";
      case "java":
        return "java";
      case "go":
        return "go";
      case "xml":
        return "xml";
      case "shell":
        return "sh";
      default:
        return "txt";
    }
  }

  function buildUntitledFilename(document: ActiveDocument): string {
    const createdAt =
      document.kind === "untitled" ? document.createdAt : Date.now();
    const stamp = new Date(createdAt)
      .toISOString()
      .replace(/:/g, "-")
      .replace(/\.\d{3}Z$/, "Z");
    return `note-${stamp}.${getPreferredExtension(activeLanguage)}`;
  }

  async function getPathLabel(path: string): Promise<string> {
    try {
      return await basename(path);
    } catch {
      return path.replace(/\\/g, "/").split("/").pop() ?? path;
    }
  }

  async function syncLanguageFromPath(path: string): Promise<void> {
    if (language !== "auto") {
      return;
    }

    const filename = await getPathLabel(path);
    const extLanguage = languageDetector.detect("", filename);
    if (extLanguage) {
      language = extLanguage;
      detectedLanguage = extLanguage;
    }
  }

  // Compute the actual language to apply to the editor
  let activeLanguage = $derived(
    language === "auto" ? detectedLanguage : language,
  );
  let activeDocument = $state.raw<ActiveDocument>(createUntitledDocument());
  let activeFilePath = $derived(getDocumentKey(activeDocument));
  let isDirty = $derived(value !== activeDocument.lastSavedValue);

  // Sync activeLanguage to global editorState
  $effect(() => {
    editorState.fileType = activeLanguage as FileType;
  });

  $effect(() => {
    editorState.currentDocumentLength = documentLength;
  });

  $effect(() => {
    editorState.isUntitledDocument = activeDocument.kind === "untitled";
  });

  $effect(() => {
    editorState.isDirty = isDirty;
  });

  $effect(() => {
    editorState.currentFilePath = activeDocument.kind === "saved" ? activeDocument.path : undefined;
  });

  $effect(() => {
    if (editorState.activeSurface === "markdown-preview") {
      return;
    }

    editorState.currentSelectionSize = selectionSize;
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
  let csvTableView = $state<
    | {
        flushToTextHistory: () => Promise<CsvTableFlushResult>;
      }
    | undefined
  >(undefined);
  let csvMirrorQueue = $state.raw<CsvMirrorTextUpdate[]>([]);
  let csvMirrorDrainHandle = $state.raw<
    | { kind: "idle"; id: number }
    | { kind: "timeout"; id: ReturnType<typeof setTimeout> }
    | undefined
  >(undefined);
  let fileOpenRequestVersion = 0;

  function beginFileOpenRequest(): number {
    fileOpenRequestVersion += 1;
    return fileOpenRequestVersion;
  }

  function invalidatePendingFileOpen(): void {
    fileOpenRequestVersion += 1;
    clearPendingSidebarOpenFile();
    void invoke("cancel_file_read").catch(() => undefined);
  }

  function isActiveFileOpenRequest(requestVersion: number): boolean {
    return requestVersion === fileOpenRequestVersion;
  }

  type IdleSchedulerWindow = Window &
    typeof globalThis & {
      requestIdleCallback?: (callback: IdleRequestCallback) => number;
      cancelIdleCallback?: (handle: number) => void;
    };

  function cancelCsvMirrorDrain(): void {
    if (csvMirrorDrainHandle === undefined || typeof window === "undefined") {
      csvMirrorDrainHandle = undefined;
      return;
    }

    const idleWindow = window as IdleSchedulerWindow;

    if (csvMirrorDrainHandle.kind === "idle" && idleWindow.cancelIdleCallback) {
      idleWindow.cancelIdleCallback(csvMirrorDrainHandle.id);
    } else if (csvMirrorDrainHandle.kind === "timeout") {
      clearTimeout(csvMirrorDrainHandle.id);
    }

    csvMirrorDrainHandle = undefined;
  }

  function applyCsvMirrorUpdate(update: CsvMirrorTextUpdate): void {
    if (!editorSession.state) {
      ensureManagedEditorState(editorSession, value, activeLanguage);
    }

    dispatchManagedEditorTextChange(editorSession, update.text, {
      userEvent: update.userEvent,
      focus: false,
      separateUndoStep: true,
    });
  }

  function drainCsvMirrorQueueSlice(deadline?: IdleDeadline): void {
    csvMirrorDrainHandle = undefined;

    let processed = 0;
    while (csvMirrorQueue.length > 0) {
      const update = csvMirrorQueue.shift();
      if (!update) {
        break;
      }

      applyCsvMirrorUpdate(update);
      processed += 1;

      if (processed >= 2) {
        break;
      }

      if (deadline && deadline.timeRemaining() < 4) {
        break;
      }
    }

    if (csvMirrorQueue.length > 0) {
      scheduleCsvMirrorDrain();
    }
  }

  function scheduleCsvMirrorDrain(): void {
    if (csvMirrorDrainHandle !== undefined || typeof window === "undefined") {
      return;
    }

    const idleWindow = window as IdleSchedulerWindow;

    if (idleWindow.requestIdleCallback) {
      csvMirrorDrainHandle = {
        kind: "idle",
        id: idleWindow.requestIdleCallback((deadline) => {
          drainCsvMirrorQueueSlice(deadline);
        }),
      };
      return;
    }

    csvMirrorDrainHandle = {
      kind: "timeout",
      id: setTimeout(() => {
        drainCsvMirrorQueueSlice();
      }, 0),
    };
  }

  async function drainCsvMirrorQueueNow(): Promise<void> {
    cancelCsvMirrorDrain();

    let processed = 0;
    while (csvMirrorQueue.length > 0) {
      const update = csvMirrorQueue.shift();
      if (!update) {
        break;
      }

      applyCsvMirrorUpdate(update);
      processed += 1;

      if (processed % 8 === 0) {
        await new Promise<void>((resolve) => setTimeout(resolve, 0));
      }
    }
  }

  function openGoToLineDialog(): void {
    if (isCsvTableActive || !editorView) {
      return;
    }

    goToLineOpen = true;
  }

  function handleCsvMirrorReset(baseText: string): void {
    cancelCsvMirrorDrain();
    csvMirrorQueue = [];

    if (!editorSession.state) {
      ensureManagedEditorState(editorSession, baseText, activeLanguage);
      return;
    }

    if (editorSession.state.doc.toString() !== baseText) {
      dispatchManagedEditorTextChange(editorSession, baseText, {
        userEvent: "table.mirror.reset",
        focus: false,
        addToHistory: false,
      });
    }
  }

  function handleCsvMirrorUpdate(update: CsvMirrorTextUpdate): void {
    csvMirrorQueue.push(update);
    scheduleCsvMirrorDrain();
  }

  function clearCsvMirrorState(): void {
    cancelCsvMirrorDrain();
    csvMirrorQueue = [];
    csvTableView = undefined;
  }

  function clearRetainedEditorState(): void {
    editorView = undefined;
    editorState.activeView = undefined;
    editorState.findReplace.visible = false;
    editorState.findReplace.findText = "";
    editorState.findReplace.replaceText = "";
    editorState.findReplace.matchCount = 0;
    editorState.findReplace.currentMatch = 0;

    // Free module-level caches that persist across file switches.
    clearSearchStatsCache();
    clearColorCache();
  }

  function resetEditorDocument(
    nextValue: string,
    nextDocument: ActiveDocument,
    nextLanguage = "auto",
    nextDetectedLanguage = "text",
  ): void {
    checkLanguage.cancel();
    clearCsvMirrorState();
    clearRetainedEditorState();

    // Eagerly release the previous file's content string (up to 200MB)
    // so it becomes GC-eligible immediately rather than lingering until
    // the old activeDocument object is collected.
    (activeDocument as Record<string, unknown>).lastSavedValue = "";

    activeDocument = nextDocument;
    editorSession = createManagedEditorSession();
    value = nextValue;
    documentLength = nextValue.length;
    lineCount = countDocumentLines(nextValue);
    line = 1;
    col = 1;
    selectionSize = 0;
    language = nextLanguage;
    detectedLanguage = nextDetectedLanguage;
    editorState.csv.showTable = false;
    editorState.activeSurface = "editor";
  }

  async function createNewFile(): Promise<void> {
    invalidatePendingFileOpen();

    const previousSession = editorSession;
    const previousDocLength = previousSession.state?.doc.length ?? value.length;

    resetEditorDocument("", createUntitledDocument());

    await new Promise<void>((resolve) => setTimeout(resolve, 10));

    disposeManagedEditorSession(previousSession);

    // Reclaim stale heap after tearing down a large document into a blank editor.
    requestFileOpenReclaim(previousDocLength, 0);
  }

  // -----------------------------------------------------------------------
  // Menu: "File > Open File..."
  //
  // The Tauri menu emits "menu://open-file" when the user clicks the item
  // (or presses Ctrl/Cmd+O via its accelerator). We open a native file
  // picker, then invoke read_file_content on the Rust side which enforces
  // the current 200 MB size limit before returning the text.
  // -----------------------------------------------------------------------
  async function openFileAtPath(filePath: string): Promise<void> {
    const requestVersion = beginFileOpenRequest();
    const filename = filePath.replace(/\\/g, "/").split("/").pop() ?? "";
    const existingPendingFile = librarySidebarState.pendingOpenFile;
    const preservesPendingMetadata = existingPendingFile?.path === filePath;

    setPendingSidebarOpenFile({
      path: filePath,
      source: preservesPendingMetadata ? existingPendingFile.source : "external",
      requestId: requestVersion,
      revealInRecentList: preservesPendingMetadata
        ? existingPendingFile.revealInRecentList
        : true,
    });

    try {
      const preparedFile = await prepareFileOpen(filePath);
      if (!isActiveFileOpenRequest(requestVersion)) {
        return;
      }

      setPendingSidebarOpenFile({
        path: filePath,
        source: preparedFile.source,
        requestId: requestVersion,
        revealInRecentList: preservesPendingMetadata
          ? existingPendingFile.revealInRecentList
          : true,
      });

      const { emit } = await import("@tauri-apps/api/event");
      await emit(RECENT_FILES_UPDATED_EVENT);
      if (!isActiveFileOpenRequest(requestVersion)) {
        return;
      }

      // Start a decelerating progress ticker after the backend has tracked the
      // file so the sidebar can refresh from SQLite before the read begins.
      startLoaderTicker("Reading file…", filename, {
        ceiling: 65,
        factor: 0.06,
        minStep: 0.3,
        interval: 80,
        startAt: 5,
      });

      const previousSession = editorSession;
      const previousDocLength =
        previousSession.state?.doc.length ?? value.length;
      const content = await invoke<string>("read_file_content", {
        path: filePath,
        requestId: requestVersion,
      });
      if (!isActiveFileOpenRequest(requestVersion)) {
        return;
      }

      stopLoaderTicker();
      updateEditorLoader("Detecting language…", filename, 72);

      // Yield to let the UI repaint before running language detection
      await new Promise<void>((r) => setTimeout(r, 0));
      if (!isActiveFileOpenRequest(requestVersion)) {
        return;
      }

      const detected = languageDetector.detect(content, filename) ?? "text";

      updateEditorLoader("Loading into editor…", filename, 88);
      await new Promise<void>((r) => setTimeout(r, 0));
      if (!isActiveFileOpenRequest(requestVersion)) {
        return;
      }

      // If the filename's extension alone resolves to a language,
      // pin it directly — no need for "auto" mode, and the debounced
      const extLang = languageDetector.detect("", filename);
      const nextLanguage = extLang ?? "auto";
      const nextDetectedLanguage = extLang ?? detected;

      resetEditorDocument(
        content,
        {
          kind: "saved",
          path: filePath,
          source: "file",
          lastSavedValue: content,
        },
        nextLanguage,
        nextDetectedLanguage,
      );

      // Yield to let Svelte update the DOM and dispose old CodeMirror instance
      await new Promise<void>((r) => setTimeout(r, 10));

      disposeManagedEditorSession(previousSession);

      // Reclaim stale heap from the previous file through the shared controller.
      requestFileOpenReclaim(previousDocLength, content.length);
      clearPendingSidebarOpenFile(requestVersion);
    } catch (err: unknown) {
      if (!isActiveFileOpenRequest(requestVersion)) {
        return;
      }

      if (err === "File read cancelled.") {
        return;
      }

      const msg = typeof err === "string" ? err : "Failed to open file.";
      toast.error(msg);
    } finally {
      if (!isActiveFileOpenRequest(requestVersion)) {
        return;
      }

      // Always clean up — idempotent in the success path
      clearPendingSidebarOpenFile(requestVersion);
      stopLoaderTicker();
      hideEditorLoader();
    }
  }

  async function openFile(): Promise<void> {
    const selected = await openFilePicker({
      multiple: false,
      directory: false,
    });

    // User cancelled the dialog
    if (!selected) return;

    await openFileAtPath(selected as string);
  }

  async function getContentForSave(): Promise<string> {
    if (activeLanguage === "csv" && editorState.csv.showTable && csvTableView) {
      if (csvInfo.liveMirrorEnabled) {
        await drainCsvMirrorQueueNow();
      } else {
        cancelCsvMirrorDrain();
        csvMirrorQueue = [];
      }

      const { text } = await csvTableView.flushToTextHistory();
      return text;
    }

    // Read directly from CM state for freshness — `value` may lag
    // behind by up to VALUE_SYNC_DEBOUNCE_MS for large documents.
    return editorSession.state?.doc.toString() ?? value;
  }

  async function writeDocumentToPath(path: string, content: string): Promise<void> {
    const filename = await getPathLabel(path);

    startLoaderTicker("Saving file…", filename, {
      ceiling: 88,
      factor: 0.08,
      minStep: 0.4,
      interval: 70,
      startAt: 8,
    });

    try {
      await invoke("write_file_content", { path, content });
      await syncLanguageFromPath(path);
      activeDocument = {
        kind: "saved",
        path,
        source: "file",
        lastSavedValue: content,
      };
      // Flush any pending debounced value sync so that `isDirty`
      // resolves immediately after saving (value === lastSavedValue).
      flushPendingValueSync(editorSession);
      const { emit } = await import("@tauri-apps/api/event");
      await emit(RECENT_FILES_UPDATED_EVENT);
    } finally {
      stopLoaderTicker();
      hideEditorLoader();
    }
  }

  async function buildManagedSavePath(): Promise<string> {
    const notesRoot = await resolveNotesRoot();
    return join(notesRoot, buildUntitledFilename(activeDocument));
  }

  async function saveFile(): Promise<void> {
    try {
      const content = await getContentForSave();

      if (activeDocument.kind === "saved") {
        if (content === activeDocument.lastSavedValue) {
          return;
        }

        await writeDocumentToPath(activeDocument.path, content);
        return;
      }

      const savePath = await buildManagedSavePath();
      await writeDocumentToPath(savePath, content);
    } catch (err: unknown) {
      const msg = typeof err === "string" ? err : "Failed to save file.";
      toast.error(msg);
    }
  }

  async function saveFileAs(): Promise<void> {
    try {
      const content = await getContentForSave();
      const defaultPath =
        activeDocument.kind === "saved"
          ? activeDocument.path
          : await buildManagedSavePath();

      const selectedPath = await saveFilePicker({
        title: "Save As",
        defaultPath,
      });

      if (!selectedPath) {
        return;
      }

      await writeDocumentToPath(selectedPath, content);
    } catch (err: unknown) {
      const msg = typeof err === "string" ? err : "Failed to save file.";
      toast.error(msg);
    }
  }

  // Register (and later clean up) the file-menu event listeners.
  $effect(() => {
    const unlistenPromise = import("@tauri-apps/api/event").then(
      async ({ listen }) => {
        const unlistenNewFile = await listen("menu://new-file", () => {
          void createNewFile();
        });
        const unlistenOpenFile = await listen("menu://open-file", () => {
          void openFile();
        });
        const unlistenOpenFilePath = await listen<OpenFilePathPayload>(OPEN_FILE_PATH_EVENT, (event) => {
          if (event.payload?.path) {
            void openFileAtPath(event.payload.path);
          }
        });
        const unlistenSaveFile = await listen("menu://save-file", () => {
          void saveFile();
        });
        const unlistenSaveFileAs = await listen("menu://save-file-as", () => {
          void saveFileAs();
        });

        return () => {
          unlistenNewFile();
          unlistenOpenFile();
          unlistenOpenFilePath();
          unlistenSaveFile();
          unlistenSaveFileAs();
        };
      },
    );

    return () => {
      unlistenPromise.then((fn) => fn());
    };
  });

  // Derive whether CSV table view is active
  let isCsvTableActive = $derived(
    activeLanguage === "csv" && editorState.csv.showTable,
  );

  let csvInfo = $state({
    rows: 0,
    cols: 0,
    delimiter: "",
    errors: 0,
    liveMirrorEnabled: false,
  });

  async function requestCsvTableMode(showTable: boolean): Promise<void> {
    if (showTable === editorState.csv.showTable) {
      return;
    }

    if (showTable) {
      editorState.csv.showTable = true;
      return;
    }

    if (activeLanguage !== "csv" || !csvTableView) {
      editorState.csv.showTable = false;
      return;
    }

    startLoaderTicker("Preparing CSV text…", "", {
      ceiling: 92,
      factor: 0.05,
      minStep: 0.2,
      interval: 80,
      startAt: 8,
      graceMs: 0,
    });

    try {
      const previousText = value;
      const useLiveMirror = csvInfo.liveMirrorEnabled;

      if (useLiveMirror) {
        await drainCsvMirrorQueueNow();
      } else {
        cancelCsvMirrorDrain();
        csvMirrorQueue = [];
      }

      const { text: nextText } = await csvTableView.flushToTextHistory();
      value = nextText;

      if (!editorSession.state) {
        ensureManagedEditorState(
          editorSession,
          useLiveMirror ? nextText : previousText,
          activeLanguage,
        );
      }

      if (editorSession.state?.doc.toString() !== nextText) {
        dispatchManagedEditorTextChange(editorSession, nextText, {
          userEvent: useLiveMirror ? "table.mirror.flush" : "flush.table",
          focus: false,
          addToHistory: useLiveMirror ? false : undefined,
        });
      }

      completeEditorLoader("CSV text ready", "", 120, () => {
        editorState.csv.showTable = false;
      });
    } catch (error) {
      stopLoaderTicker();
      hideEditorLoader();
      toast.error(
        error instanceof Error ? error.message : "Failed to prepare CSV text.",
      );
    }
  }

  $effect(() => {
    return registerHotkey(
      "Mod+G",
      (event) => {
        if (isCsvTableActive || !editorView) {
          return;
        }

        event.preventDefault();
        event.stopPropagation();
        openGoToLineDialog();
      },
      { ignoreInputs: false },
    );
  });

  $effect(() => {
    editorState.goToLine.requestOpen = openGoToLineDialog;

    return () => {
      if (editorState.goToLine.requestOpen === openGoToLineDialog) {
        editorState.goToLine.requestOpen = undefined;
      }
    };
  });

  $effect(() => {
    editorState.csv.requestShowTable = requestCsvTableMode;

    return () => {
      invalidatePendingFileOpen();
      checkLanguage.cancel();
      clearCsvMirrorState();
      clearRetainedEditorState();
      // Eagerly release large strings before session dispose.
      (activeDocument as Record<string, unknown>).lastSavedValue = "";
      value = "";
      disposeManagedEditorSession(editorSession);
      if (editorState.goToLine.requestOpen === openGoToLineDialog) {
        editorState.goToLine.requestOpen = undefined;
      }
      if (editorState.csv.requestShowTable === requestCsvTableMode) {
        editorState.csv.requestShowTable = undefined;
      }
    };
  });
</script>

<div class="flex flex-1 flex-col min-h-0 min-w-0">
  <GoToLineDialog bind:open={goToLineOpen} {editorView} {line} {lineCount} />

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
            bind:this={csvTableView}
            bind:content={value}
            bind:tableInfo={csvInfo}
            onMirrorReset={handleCsvMirrorReset}
            onMirrorUpdate={handleCsvMirrorUpdate}
          />
        </div>
      {:else}
        <div class="relative flex-1 min-h-0 min-w-0">
          <div class="absolute inset-0">
            {#key activeFilePath}
              <Editor
                bind:value
                bind:documentLength
                bind:lineCount
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
          <div
            class="split-surface relative h-full min-h-0 min-w-0"
            data-active={editorState.activeSurface === "editor" && editorState.markdown.showPreview}
          >
            <div class="absolute inset-0">
              {#key activeFilePath}
                <Editor
                  bind:value
                  bind:documentLength
                  bind:lineCount
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
        </ResizablePane>

        {#if editorState.markdown.showPreview}
          <ResizableHandle />
          <ResizablePane
            defaultSize={50}
            minSize={15}
            class="flex flex-col min-h-0 min-w-0"
          >
            <div
              class="split-surface flex flex-col flex-1 min-h-0 min-w-0"
              data-active={editorState.activeSurface === "markdown-preview"}
            >
              <MarkdownPreview content={value} {editorView} />
            </div>
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
              bind:documentLength
              bind:lineCount
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
    {documentLength}
    {lineCount}
    {line}
    {col}
    {selectionSize}
    bind:language
    {detectedLanguage}
    {activeLanguage}
    {isCsvTableActive}
    {csvInfo}
    onGoToLine={openGoToLineDialog}
  />
</div>

<style>
  .split-surface {
    position: relative;
  }

  /* Thin top-edge active indicator, hidden by default */
  .split-surface::before {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 2px;
    z-index: 10;
    pointer-events: none;
    background: var(--ring);
    opacity: 0;
    transition: opacity 140ms ease;
  }

  .split-surface[data-active="true"]::before {
    opacity: 0.6;
  }
</style>
