<script lang="ts">
  import Editor from "$lib/editor/components/Editor.svelte";
  import MarkdownPreview from "$lib/editor/components/markdown/MarkdownPreview.svelte";
  import CsvTableView from "./csv/CsvTableView.svelte";
  import StatusBar from "$lib/editor/components/StatusBar.svelte";
  import EditorLoader from "$lib/editor/components/EditorLoader.svelte";
  import GoToLineDialog from "$lib/editor/components/GoToLineDialog.svelte";
  import IndentationPicker, { type IndentConfig, type IndentSelection } from "$lib/editor/components/IndentationPicker.svelte";
  import TransformationsPalette from "$lib/editor/components/TransformationsPalette.svelte";
  import {
    ResizablePaneGroup,
    ResizablePane,
    ResizableHandle,
  } from "$lib/components/ui/resizable";
  import { detectByFilename } from "$lib/ipc";
  import { debounce } from "lodash-es";
  import type { EditorView } from "codemirror";
  import { Text } from "@codemirror/state";
  import {
    createManagedEditorSession,
    dispatchManagedEditorChange,
    dispatchManagedEditorTextChange,
    disposeManagedEditorSession,
    ensureManagedEditorState,
    flushPendingValueSync,
    setManagedEditorIndent,
    type ManagedEditorSession,
  } from "$lib/editor/core/editorSession";
  import {
    type CsvMirrorTextUpdate,
    type CsvTableFlushResult,
  } from "./csv/csvTableProtocol";
  import {
    closeEditorPopup,
    editorState,
    openEditorPopup,
    openGoToLinePanel,
    registerEditorPopup,
    syncEditorPopupOpenState,
    hideEditorLoader,
    updateEditorLoader,
    startLoaderTicker,
    stopLoaderTicker,
    completeEditorLoader,
    type FileType,
  } from "$lib/state/editor.svelte";
  import {
    basename,
  } from "@tauri-apps/api/path";
  import { createChunkedTextAccumulator, invoke, invokeText } from "$lib/ipc";
  import { Channel } from "@tauri-apps/api/core";
  import { toast } from "$lib/components/ui/sonner";
  import { requestFileOpenReclaim } from "$lib/editor/core/memory";
  import { clearSearchStatsCache, editorGoToLine } from "$lib/editor/core/actions";
  import { clearColorCache } from "$lib/editor/extensions/colorHints";
  import {
    librarySidebarState,
    clearPendingSidebarOpenFile,
    reportLibraryMutation,
    setPendingSidebarOpenFile,
  } from "$lib/state/librarySidebar.svelte";
  import { confirmBeforeLeavingDocument } from "$lib/state/unsavedChangesGuard.svelte";
  import {
    OPEN_FILE_PATH_EVENT,
    DOCUMENT_RENAMED_EVENT,
    RESET_TO_BLANK_EVENT,
    type DocumentDescriptor,
    type OpenFilePathPayload,
    type RecentFileSource,
  } from "$lib/files/recentFiles";
  import { onMount, untrack } from "svelte";
  import {
    appSettingsState,
    loadAllSettings,
    saveLastActiveDocument,
  } from "$lib/state/appSettings.svelte";
  import {
    type ExecuteTransformationResponse,
    type ExecuteTransformationRequest,
    getTransformationAction,
    type TransformationActionId,
    type TransformationMessageLevel,
    type TransformationChannelEvent,
  } from "$lib/transformations/actions";

  type SavedDocumentSource = "slates" | "local";

  type ActiveDocument =
    | {
        kind: "untitled";
        key: string;
        createdAt: number;
        lastSavedValue: string;
        source: "slates";
      }
    | {
        kind: "saved";
        documentId: string;
        documentGeneration: number;
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

  // Seed the indent selection for a brand-new/opened document as "follow the
  // global default" — a real, persisted choice (not a one-time copy of
  // concrete values) so the IndentationPicker can show "Default" as selected
  // until the user explicitly overrides it.
  function resolveDefaultIndentConfig(): IndentSelection {
    return {
      indentMode: "default",
      indentSize: appSettingsState.defaultIndentSize,
    };
  }

  let indentSelection = $state<IndentSelection>(resolveDefaultIndentConfig());
  let indentPickerOpen = $state(false);

  // Concrete indentation config for actual consumers (CodeMirror, status
  // bar). Resolves "default" live from the global setting, so a document set
  // to "Default" tracks Settings changes made while it's open instead of
  // freezing whatever value was baked in at open/pick time.
  const effectiveIndentConfig = $derived<IndentConfig>(
    indentSelection.indentMode === "default"
      ? { indentMode: appSettingsState.defaultIndentMode, indentSize: appSettingsState.defaultIndentSize }
      : { indentMode: indentSelection.indentMode, indentSize: indentSelection.indentSize },
  );

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

  function buildCodeMirrorTextFromChunks(chunks: string[]): Text {
    let doc = Text.empty;
    let previousChunkEndedWithCR = false;

    for (let index = 0; index < chunks.length; index += 1) {
      let chunk = chunks[index] ?? "";
      chunks[index] = "";

      if (previousChunkEndedWithCR && chunk.startsWith("\n")) {
        chunk = chunk.slice(1);
      }

      previousChunkEndedWithCR = chunk.endsWith("\r");
      doc = doc.append(Text.of(chunk.split(/\r\n?|\n/)));
    }

    chunks.length = 0;
    return doc;
  }

  function createUntitledDocument(now = Date.now()): ActiveDocument {
    return {
      kind: "untitled",
      key: `untitled:${now}`,
      createdAt: now,
      lastSavedValue: "",
      source: "slates",
    };
  }

  function getDocumentKey(document: ActiveDocument): string {
    return document.kind === "untitled" ? document.key : document.documentId;
  }

  async function getPathLabel(path: string): Promise<string> {
    try {
      return await basename(path);
    } catch {
      return path.replace(/\\/g, "/").split("/").pop() ?? path;
    }
  }

  async function syncLanguageFromPath(path: string): Promise<void> {
    const filename = await getPathLabel(path);
    const extLanguage = await detectByFilename(filename);
    if (extLanguage) {
      // Pin language to the extension of the saved file so the status bar
      // always reflects the actual file type after a save or save-as.
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
  // Managed slates are persisted by the backend autosave flow, including
  // untitled slates that are flushed before a document switch. "Dirty" is
  // therefore reserved for local files that need an explicit save.
  let isDirty = $derived(
    activeDocument.source === "local" && value !== activeDocument.lastSavedValue,
  );

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
    editorState.currentDocumentId = activeDocument.kind === "saved"
      ? activeDocument.documentId
      : undefined;
    editorState.currentDocumentGeneration = activeDocument.kind === "saved"
      ? activeDocument.documentGeneration
      : undefined;
  });

  $effect(() => {
    editorState.currentFileSource = activeDocument.source;
  });

  $effect(() => {
    editorState.requestSaveCurrentDocument = saveFile;
    return () => {
      editorState.requestSaveCurrentDocument = undefined;
    };
  });

  $effect(() => {
    if (editorState.activeSurface === "markdown-preview") {
      return;
    }

    editorState.currentSelectionSize = selectionSize;
  });

  const checkLanguage = debounce(async (content: string) => {
    if (language === "auto" && content) {
      const result = await invoke<string | null>("detect_language", {
        content,
        filename: null,
      });
      if (result) {
        detectedLanguage = result;
      }
    }
  }, 1000);

  // Watch the `value` and run language detection when it changes
  $effect(() => {
    checkLanguage(value);
  });

  // -----------------------------------------------------------------------
  // Backend-driven autosave integration
  //
  // For slate files, Rust owns save scheduling. The frontend's role:
  //   1. Send lightweight `autosave_notify_changed(generation)` on edits
  //   2. Respond to `autosave://request-content` with serialized content
  //   3. Send content before file switches via `autosave_flush_before_switch`
  //   4. Handle `autosave://document-created` for untitled→saved transitions
  // -----------------------------------------------------------------------

  let autosaveGeneration = 0;
  let previousAutosaveValue = "";

  // Notify Rust when the editor content changes (piggybacked on VALUE_SYNC).
  // This is lightweight — only a u64 generation crosses IPC, no content.
  $effect(() => {
    // Reading `value` creates the reactive subscription.
    const currentValue = value;
    if (currentValue !== previousAutosaveValue) {
      previousAutosaveValue = currentValue;
      if (activeDocument.source === "slates") {
        autosaveGeneration += 1;
        invoke("autosave_notify_changed", { generation: autosaveGeneration }).catch(() => {});
      }
    }
  });

  // Rust creates saved-document autosave sessions from an authorized document
  // grant. The frontend can only activate a pathless untitled slate; it never
  // supplies a filesystem path or source classification.
  $effect(() => {
    if (activeDocument.kind === "untitled") {
      const key = activeDocument.key;
      void key;
      const languageHint = untrack(() => language);
      invoke("autosave_activate_untitled", { languageHint }).catch(() => {});
    }
  });

  $effect(() => {
    invoke("autosave_set_language_hint", { languageHint: language }).catch(() => {});
  });

  // Listen for autosave events from Rust.
  $effect(() => {
    const unlistenPromise = import("@tauri-apps/api/event").then(
      async ({ listen }) => {
        // Rust requests content when it decides to save
        const unlistenRequestContent = await listen<{ requestId: number }>(
          "autosave://request-content",
          async (event) => {
            const content = editorSession.state?.doc.toString() ?? value;
            invoke("autosave_submit_content", {
              requestId: event.payload.requestId,
              generation: autosaveGeneration,
              content,
            }).catch((err) => console.error("Autosave submit failed:", err));
          },
        );

        // Rust created a new file for an untitled slate
        const unlistenDocumentCreated = await listen<{
          path: string;
          documentId: string;
          documentGeneration: number;
          detectedLanguage: string;
        }>("autosave://document-created", async (event) => {
          const {
            path,
            documentId,
            documentGeneration,
            detectedLanguage: lang,
          } = event.payload;
          if (
            activeDocument.kind === "saved" &&
            activeDocument.documentId === documentId &&
            activeDocument.documentGeneration === documentGeneration &&
            activeDocument.path === path
          ) {
            return;
          }
          if (activeDocument.kind === "saved") {
            return;
          }
          await syncLanguageFromPath(path);
          if (language === "auto" && lang) {
            detectedLanguage = lang;
          }
          activeDocument = {
            kind: "saved",
            documentId,
            documentGeneration,
            path,
            source: "slates",
            lastSavedValue: value,
          };
          flushPendingValueSync(editorSession);
          reportLibraryMutation({ kind: "created", path, source: "slates" });
          // The {#key activeFilePath} block destroys and remounts <Editor>
          // when the document key changes (untitled key → saved path),
          // which drops DOM focus even though the CodeMirror selection is
          // preserved in `editorSession`. Wait a tick for Svelte to mount
          // the new EditorView, then restore focus so the caret stays
          // visible — mirrors the same workaround in `saveFile()`.
          await new Promise<void>((resolve) => setTimeout(resolve, 10));
          editorView?.focus();
        });

        // Rust asks FE to flush before window close
        const unlistenFlushClose = await listen<{ requestId: number }>(
          "autosave://flush-before-close",
          async (event) => {
            const content = editorSession.state?.doc.toString() ?? value;
            invoke("autosave_submit_content", {
              requestId: event.payload.requestId,
              generation: autosaveGeneration,
              content,
            }).catch(() => {});
          },
        );

        return () => {
          unlistenRequestContent();
          unlistenDocumentCreated();
          unlistenFlushClose();
        };
      },
    );

    return () => {
      unlistenPromise.then((fn) => fn()).catch(() => {});
    };
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

  function showTransformationToast(
    level: TransformationMessageLevel,
    message: string,
  ): void {
    switch (level) {
      case "error":
        toast.error(message);
        return;
      case "info":
        toast.info(message);
        return;
      case "success":
        toast.success(message);
        return;
    }
  }

  // Monotonically incrementing ID for each transformation invocation.
  // Used to correlate frontend cancellation requests with the Rust registry.
  let transformationRequestCounter = 0;

  // Cancel handler for the currently in-flight transformation, or null when idle.
  // Passed to EditorLoader so the cancel button can abort the Rust task.
  let transformationCancelFn = $state<(() => void) | null>(null);

  function beginTransformationCancellation(
    requestId: number,
    markCancelled: () => void,
  ): void {
    transformationCancelFn = () => {
      markCancelled();
      void invoke("cancel_transformation", { requestId }).catch(() => undefined);
    };
  }

  function endTransformationCancellation(): void {
    transformationCancelFn = null;
  }

  async function executeTransformation(actionId: TransformationActionId): Promise<boolean> {
    const action = getTransformationAction(actionId);
    if (!action) {
      toast.error("Unknown transformation.");
      return false;
    }

    if (editorState.loader.visible) {
      return false;
    }

    if (!editorSession.state) {
      toast.error("No editor document is ready for transformations.");
      return false;
    }

    const selection = editorSession.state.selection.main;
    const applyAsInsert = action.applyMode === "insert";
    const useSelection = action.supportsSelection && !selection.empty;
    let sourceText = "";
    if (!applyAsInsert) {
      sourceText = useSelection
        ? editorSession.state.doc.sliceString(selection.from, selection.to)
        : editorSession.state.doc.toString();
    }

    const requestId = ++transformationRequestCounter;
    const isFormatAction = actionId.endsWith(".format");
    const request: ExecuteTransformationRequest = {
      actionId,
      text: sourceText,
      requestId,
      ...(isFormatAction ? { params: { indentConfig: effectiveIndentConfig } } : {}),
    };

    // Track whether the user explicitly cancelled so we suppress the error toast.
    let userCancelled = false;

    // Large transformation results travel on the channel in chunked form.
    // The command response itself is just a small envelope that tells us what
    // happened and how many chunks to wait for before touching CodeMirror.
    const chunkAccumulator = createChunkedTextAccumulator();

    const onEvent = new Channel<TransformationChannelEvent>();
    onEvent.onmessage = (event) => {
      if (event.type === "progress") {
        editorState.loader.progress = Math.round(
          (event.current / Math.max(event.total, 1)) * 100,
        );
      } else {
        chunkAccumulator.handleChunk(event);
      }
    };

    // Grace-period loader: prepare state at 0 % but only show the overlay
    // after 150 ms.  Fast transforms finish before the overlay appears.
    editorState.loader.message = action.title + "…";
    editorState.loader.subMessage = "";
    editorState.loader.progress = 0;
    const graceTimeout = setTimeout(() => {
      editorState.loader.visible = true;
    }, 150);

    try {
      beginTransformationCancellation(requestId, () => {
        userCancelled = true;
      });

      const result = await invoke<ExecuteTransformationResponse>(
        "execute_transformation",
        { request, onEvent },
      );

      if (result.kind === "show-message") {
        showTransformationToast(result.level, result.message);
        editorView?.focus();
        return true;
      }

      const resultChunks = await chunkAccumulator.waitForChunks(result.chunkCount);
      // Build a CodeMirror rope from the chunks instead of creating one giant
      // JS string, which would throw "Invalid string length" for ~400 MB results.
      const resultDoc = buildCodeMirrorTextFromChunks(resultChunks);

      if (applyAsInsert || useSelection) {
        dispatchManagedEditorChange(
          editorSession,
          {
            from: selection.from,
            to: selection.to,
            insert: resultDoc,
          },
          {
            userEvent: `input.transform.${actionId}`,
            separateUndoStep: true,
            // CodeMirror's default mapping keeps a cursor at the start when
            // text is inserted at a collapsed range. Generators should leave
            // the cursor after the generated text; replacement transforms
            // retain their existing selection mapping.
            ...(applyAsInsert
              ? { selection: { anchor: selection.from + resultDoc.length } }
              : {}),
          },
        );
      } else {
        // If the action declares an output language, pin the editor to it
        // directly — we know exactly what the transform produced.
        if (action.outputLanguage) {
          language = action.outputLanguage;
          detectedLanguage = action.outputLanguage;
        }
        // Replace the full document as one transaction — bypass getMinimalTextChange
        // (which would call doc.toString() on the old doc, needlessly allocating the
        // entire pre-transform text in memory alongside the new result).
        const oldDocLength = editorSession.state?.doc.length ?? 0;
        dispatchManagedEditorChange(
          editorSession,
          { from: 0, to: oldDocLength, insert: resultDoc },
          {
            userEvent: `input.transform.${actionId}`,
            separateUndoStep: true,
          },
        );
      }

      if (result.message) {
        showTransformationToast(result.level ?? "success", result.message);
      }

      return true;
    } catch (error) {
      if (!userCancelled) {
        const message = error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : "Failed to run transformation.";
        toast.error(message);
      }
      return false;
    } finally {
      chunkAccumulator.reset();
      clearTimeout(graceTimeout);
      endTransformationCancellation();
      completeEditorLoader();
    }
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

  function requestGoToLineDialogOpen(): boolean {
    if (isCsvTableActive || !editorView) {
      return false;
    }

    return openEditorPopup({ id: "go-to-line" });
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
    closeEditorPopup();
    goToLineOpen = false;
    indentPickerOpen = false;
    editorView = undefined;
    editorState.activeView = undefined;
    editorState.findReplace.findText = "";
    editorState.findReplace.replaceText = "";
    editorState.findReplace.matchCount = 0;
    editorState.findReplace.currentMatch = 0;
    editorState.findReplace.searchError = "";

    // Free module-level caches that persist across file switches.
    clearSearchStatsCache();
    clearColorCache();
  }

  async function resetEditorDocument(
    nextValue: string,
    nextDocument: ActiveDocument,
    nextLanguage = "auto",
    nextDetectedLanguage = "text",
  ): Promise<void> {
    // Flush unsaved slate content to Rust before switching documents.
    if (activeDocument.source === "slates") {
      const content = editorSession.state?.doc.toString() ?? value;
      const gen = autosaveGeneration;
      await invoke("autosave_flush_before_switch", {
        content,
        generation: gen,
      }).catch((err) => console.error("Autosave flush on switch failed:", err));
    }

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
    indentSelection = resolveDefaultIndentConfig();
    editorState.csv.showTable = false;
    editorState.activeSurface = "editor";

    // Reset autosave generation for the new document
    autosaveGeneration = 0;
    previousAutosaveValue = nextValue;
  }

  // Resets the editor to a blank untitled slate. Does NOT confirm unsaved
  // changes itself — callers that can discard user content must run
  // `confirmBeforeLeavingDocument()` first (either here via `createNewFile`,
  // or upstream before emitting `RESET_TO_BLANK_EVENT`).
  async function resetToBlankDocument(): Promise<void> {
    // Clear the last-active pointer so "reopen last file" doesn't resurrect
    // the file the user just navigated away from. (Also covers deleting/
    // unlinking the current file, which route here.)
    saveLastActiveDocument(null);

    invalidatePendingFileOpen();

    const previousSession = editorSession;
    const previousDocLength = previousSession.state?.doc.length ?? value.length;

    await resetEditorDocument("", createUntitledDocument());

    await new Promise<void>((resolve) => setTimeout(resolve, 10));

    disposeManagedEditorSession(previousSession);

    // Focus the editor so the user can start typing immediately.
    editorView?.focus();

    // Reclaim stale heap after tearing down a large document into a blank editor.
    requestFileOpenReclaim(previousDocLength, 0);
  }

  async function createNewFile(): Promise<void> {
    if (!(await confirmBeforeLeavingDocument())) return;
    await resetToBlankDocument();
  }

  // -----------------------------------------------------------------------
  // Menu: "File > Open File..."
  //
  // The Tauri menu emits "menu://open-file" when the user clicks the item
  // (or presses Ctrl/Cmd+O via its accelerator). We open a native file
  // picker, then invoke read_file_content on the Rust side which enforces
  // the current 200 MB size limit before returning the text.
  // -----------------------------------------------------------------------
  async function openAuthorizedDocument(
    document: DocumentDescriptor,
    lineNumber?: number,
    options?: { silent?: boolean },
  ): Promise<void> {
    const filePath = document.displayPath;
    // Fast path: the file is already loaded — avoid a full reload and just
    // navigate to the requested line directly.
    if (editorState.currentFilePath === filePath && editorView) {
      if (lineNumber !== undefined) {
        editorGoToLine(editorView, lineNumber);
      }
      // Clean up any pending sidebar state that openRecentFile may have set
      // so it doesn't linger and block the editor-navigation effect later.
      clearPendingSidebarOpenFile();
      return;
    }

    const requestVersion = beginFileOpenRequest();
    const filename = filePath.replace(/\\/g, "/").split("/").pop() ?? "";
    const existingPendingFile = librarySidebarState.pendingOpenFile;
    const preservesPendingMetadata = existingPendingFile?.path === filePath;
    const revealInRecentList = preservesPendingMetadata
      ? existingPendingFile.revealInRecentList
      : true;
    const openOrigin = revealInRecentList ? "local" : "sidebar";

    setPendingSidebarOpenFile({
      path: filePath,
      source: document.source,
      requestId: requestVersion,
      revealInRecentList,
      lineNumber,
    });

    try {
      // Start a decelerating progress ticker while the file is read. The
      // backend records only first-time opens so new local files appear in
      // the Local sidebar tab without bumping timestamps when tracked files are
      // reopened.
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
      const content = await invokeText("read_file_content", {
        documentId: document.documentId,
        documentGeneration: document.generation,
        requestId: requestVersion,
      });
      if (!isActiveFileOpenRequest(requestVersion)) {
        return;
      }

      stopLoaderTicker();
      updateEditorLoader("Loading into editor…", filename, 80);

      // Yield to let the UI repaint before loading into the editor.
      await new Promise<void>((r) => setTimeout(r, 0));
      if (!isActiveFileOpenRequest(requestVersion)) {
        return;
      }

      // Use extension/filename-only detection — no content scan needed on open.
      // The file extension is the authoritative source of language type on first
      // load. Content-based detection runs later only when a full-document
      // transformation resets the mode to "auto".
      const detected = (await detectByFilename(filename)) ?? "text";

      const nextLanguage = detected;
      const nextDetectedLanguage = detected;

      await resetEditorDocument(
        content,
        {
          kind: "saved",
          documentId: document.documentId,
          documentGeneration: document.generation,
          path: filePath,
          source: document.source,
          lastSavedValue: content,
        },
        nextLanguage,
        nextDetectedLanguage,
      );
      await invoke("autosave_activate_document", {
        documentId: document.documentId,
        documentGeneration: document.generation,
        languageHint: language,
      });

      reportLibraryMutation({
        kind: "opened",
        path: filePath,
        source: document.source,
        origin: openOrigin,
      });

      // Remember this as the last-active file for the "reopen last file"
      // startup behavior. Fire-and-forget — best-effort convenience only.
      saveLastActiveDocument(document);

      // Yield to let Svelte update the DOM and dispose old CodeMirror instance
      await new Promise<void>((r) => setTimeout(r, 10));

      // Navigate to the requested line after the editor view is initialized.
      if (lineNumber !== undefined && editorView) {
        editorGoToLine(editorView, lineNumber);
      }

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

      // Startup restoration passes `silent` so a since-deleted last-active file
      // fails quietly to the default blank slate instead of nagging on launch.
      if (options?.silent) {
        console.warn(`[Startup] Could not reopen last file "${filePath}":`, err);
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
    if (!(await confirmBeforeLeavingDocument())) return;

    const selected = await invoke<DocumentDescriptor | null>("pick_document");

    // User cancelled the dialog
    if (!selected) return;

    await openAuthorizedDocument(selected);
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

  type SaveActionKind = "save" | "save-as";

  let saveActionInFlight: Promise<boolean> | undefined;
  let trailingSaveRequested = false;

  function currentDocumentNeedsSave(): boolean {
    if (activeLanguage === "csv" && editorState.csv.showTable) {
      return true;
    }

    const currentContent = editorSession.state?.doc.toString() ?? value;
    return activeDocument.kind === "untitled"
      ? true
      : currentContent !== activeDocument.lastSavedValue;
  }

  function snapshotActiveDocument(): ActiveDocument {
    return activeDocument;
  }

  function requestSaveAction(kind: SaveActionKind): Promise<boolean> {
    if (saveActionInFlight) {
      if (kind === "save") {
        trailingSaveRequested = true;
      }
      return saveActionInFlight;
    }

    const operation = runSaveAction(kind).finally(() => {
      if (saveActionInFlight === operation) {
        saveActionInFlight = undefined;
      }
    });
    saveActionInFlight = operation;
    return operation;
  }

  async function runSaveAction(kind: SaveActionKind): Promise<boolean> {
    editorState.saveInProgress = true;
    try {
      let succeeded = kind === "save"
        ? await performSaveFile()
        : await performSaveFileAs();

      // Keep one pending slot rather than queueing every key repeat. If more
      // edits and Save requests arrive during the trailing write, loop once
      // more with the newest content while still preserving strict ordering.
      while (trailingSaveRequested) {
        trailingSaveRequested = false;
        if (currentDocumentNeedsSave()) {
          succeeded = await performSaveFile();
        }
      }
      return succeeded;
    } finally {
      trailingSaveRequested = false;
      editorState.saveInProgress = false;
    }
  }

  async function writeDocument(
    document: DocumentDescriptor,
    content: string,
    expectedDocumentKey = getDocumentKey(activeDocument),
  ): Promise<DocumentDescriptor> {
    const previousPath = activeDocument.kind === "saved" ? activeDocument.path : undefined;
    const saved = await invoke<DocumentDescriptor>("write_file_content", {
      documentId: document.documentId,
      documentGeneration: document.generation,
      content,
    });

    if (getDocumentKey(activeDocument) !== expectedDocumentKey) {
      return saved;
    }
    await syncLanguageFromPath(saved.displayPath);

    activeDocument = {
      kind: "saved",
      documentId: saved.documentId,
      documentGeneration: saved.generation,
      path: saved.displayPath,
      source: saved.source,
      lastSavedValue: content,
    };
    if (previousPath !== saved.displayPath) {
      reportLibraryMutation({ kind: "created", path: saved.displayPath, source: saved.source });
    }
    // Flush any pending debounced value sync so that `isDirty`
    // resolves immediately after saving (value === lastSavedValue).
    flushPendingValueSync(editorSession);
    saveLastActiveDocument(saved);
    return saved;
  }

  /**
   * First save of an untitled document: Rust picks a smart content-based
   * filename, writes the file, and returns the final absolute path.
   *
   * When the editor is in "auto" mode, the backend auto-detects the
   * language from content (no separate frontend detection needed).
   */
  async function saveUntitledSlate(content: string): Promise<{
    descriptor: DocumentDescriptor;
    detectedLanguage: string;
  }> {
    const result = await invoke<{
      path: string;
      documentId: string;
      documentGeneration: number;
      source: RecentFileSource;
      detectedLanguage: string;
    }>(
      "save_untitled_slate",
      {
        content,
        languageHint: language,
      },
    );

    return {
      descriptor: {
        documentId: result.documentId,
        generation: result.documentGeneration,
        displayPath: result.path,
        fileName: await getPathLabel(result.path),
        source: result.source,
        writable: true,
      },
      detectedLanguage: result.detectedLanguage,
    };
  }

  function saveFile(): Promise<boolean> {
    return requestSaveAction("save");
  }

  async function performSaveFile(): Promise<boolean> {
    try {
      const expectedDocumentKey = getDocumentKey(activeDocument);
      const content = await getContentForSave();

      if (activeDocument.kind === "saved") {
        if (content === activeDocument.lastSavedValue) {
          return true;
        }

        await writeDocument({
          documentId: activeDocument.documentId,
          generation: activeDocument.documentGeneration,
          displayPath: activeDocument.path,
          fileName: await getPathLabel(activeDocument.path),
          source: activeDocument.source,
          writable: true,
        }, content, expectedDocumentKey);
        editorView?.focus();
        return true;
      }

      const saved = await saveUntitledSlate(content);
      const savePath = saved.descriptor.displayPath;
      const currentDocument = snapshotActiveDocument();
      const alreadyApplied = currentDocument.kind === "saved" &&
        currentDocument.documentId === saved.descriptor.documentId &&
        currentDocument.path === savePath;
      if (!alreadyApplied) {
        reportLibraryMutation({ kind: "created", path: savePath, source: "slates" });
      }
      if (alreadyApplied || getDocumentKey(activeDocument) !== expectedDocumentKey) {
        return true;
      }
      if (language === "auto" && saved.detectedLanguage) {
        detectedLanguage = saved.detectedLanguage;
      }
      // Transition the document state — writeDocumentToPath would overwrite
      // with write_file_content again, so apply state directly here.
      await syncLanguageFromPath(savePath);
      activeDocument = {
        kind: "saved",
        documentId: saved.descriptor.documentId,
        documentGeneration: saved.descriptor.generation,
        path: savePath,
        source: "slates",
        lastSavedValue: content,
      };
      // A freshly-saved untitled slate is now a real file — track it as last-active.
      saveLastActiveDocument(saved.descriptor);
      flushPendingValueSync(editorSession);
      // The {#key activeFilePath} block destroys and remounts <Editor> when
      // the document key changes (untitled key → saved path). Wait a tick
      // for Svelte to mount the new EditorView before focusing.
      await new Promise<void>((resolve) => setTimeout(resolve, 10));
      editorView?.focus();
      return true;
    } catch (err: unknown) {
      const msg = typeof err === "string" ? err : "Failed to save file.";
      toast.error(msg);
      return false;
    }
  }

  async function saveFileAs(): Promise<void> {
    await requestSaveAction("save-as");
  }

  async function performSaveFileAs(): Promise<boolean> {
    try {
      const expectedDocumentKey = getDocumentKey(activeDocument);
      const content = await getContentForSave();

      let suggestedName: string | undefined;
      if (activeDocument.kind === "saved") {
        suggestedName = undefined;
      } else {
        // Ask Rust for a smart suggested filename (no collision check, no write).
        const suggestion = await invoke<{ filename: string; detectedLanguage: string }>(
          "suggest_slate_name",
          {
            content,
            // Same as saveUntitledSlate: send language (not activeLanguage)
            // so the backend can re-detect when in auto mode.
            languageHint: language,
          },
        );
        suggestedName = suggestion.filename;
      }

      const selected = await invoke<DocumentDescriptor | null>("pick_save_document", {
        currentDocumentId: activeDocument.kind === "saved" ? activeDocument.documentId : null,
        currentDocumentGeneration: activeDocument.kind === "saved"
          ? activeDocument.documentGeneration
          : null,
        suggestedName: suggestedName ?? null,
      });

      if (!selected) {
        return true;
      }

      await writeDocument(selected, content, expectedDocumentKey);
      return true;
    } catch (err: unknown) {
      const msg = typeof err === "string" ? err : "Failed to save file.";
      toast.error(msg);
      return false;
    }
  }

  // Startup: optionally reopen the last-active file. EditorWrapper owns all
  // document transitions, so it also owns this one-time restoration decision
  // rather than coupling to +layout.svelte's settings-load timing. We read the
  // settings directly (one cheap IPC call) instead of depending on when the
  // layout's hydrate runs. On any failure we silently keep the default blank
  // slate that `activeDocument` already initialized to.
  onMount(async () => {
    try {
      const settings = await loadAllSettings();
      if (settings.startupBehavior === "last") {
        const lastDocument = await invoke<DocumentDescriptor | null>("get_last_active_document");
        if (lastDocument) {
          await openAuthorizedDocument(lastDocument, undefined, { silent: true });
        }
      }
    } catch (err) {
      console.warn("[Startup] Failed to evaluate startup-file behavior:", err);
    }
  });

  // Register (and later clean up) the file-menu event listeners.
  $effect(() => {
    const unlistenPromise = import("@tauri-apps/api/event").then(
      async ({ listen }) => {
        const unlistenNewFile = await listen("menu://new-file", () => {
          void createNewFile();
        });
        const unlistenResetToBlank = await listen(RESET_TO_BLANK_EVENT, () => {
          void resetToBlankDocument();
        });
        const unlistenOpenFile = await listen("menu://open-file", () => {
          void openFile();
        });
        const unlistenOpenFilePath = await listen<OpenFilePathPayload>(OPEN_FILE_PATH_EVENT, (event) => {
          if (event.payload?.documentId) {
            void openAuthorizedDocument({
              documentId: event.payload.documentId,
              generation: event.payload.documentGeneration,
              displayPath: event.payload.path,
              fileName: event.payload.path.replace(/\\/g, "/").split("/").pop() ?? "",
              source: event.payload.source ?? "local",
              writable: true,
            }, event.payload.lineNumber);
          }
        });
        const unlistenDocumentRenamed = await listen<DocumentDescriptor>(DOCUMENT_RENAMED_EVENT, (event) => {
          if (
            activeDocument.kind === "saved" &&
            activeDocument.documentId === event.payload.documentId
          ) {
            activeDocument = {
              ...activeDocument,
              documentGeneration: event.payload.generation,
              path: event.payload.displayPath,
              source: event.payload.source,
            };
            saveLastActiveDocument(event.payload);
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
          unlistenResetToBlank();
          unlistenOpenFile();
          unlistenOpenFilePath();
          unlistenDocumentRenamed();
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

    // Notify autosave backend of CSV mode changes
    invoke("autosave_set_csv_mode", { active: showTable }).catch(() => {});

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
    editorState.goToLine.requestOpen = requestGoToLineDialogOpen;

    return () => {
      if (editorState.goToLine.requestOpen === requestGoToLineDialogOpen) {
        editorState.goToLine.requestOpen = undefined;
      }
    };
  });

  $effect(() => {
    syncEditorPopupOpenState("go-to-line", goToLineOpen);
  });

  $effect(() => {
    return registerEditorPopup("go-to-line", {
      open: () => {
        goToLineOpen = true;
      },
      close: () => {
        goToLineOpen = false;
      },
    });
  });

  function openIndentPicker(): boolean {
    if (isCsvTableActive) return false;
    indentPickerOpen = true;
    return true;
  }

  $effect(() => {
    syncEditorPopupOpenState("indentation-picker", indentPickerOpen);
  });

  $effect(() => {
    return registerEditorPopup("indentation-picker", {
      open: () => {
        indentPickerOpen = true;
      },
      close: () => {
        indentPickerOpen = false;
      },
    });
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
      if (editorState.goToLine.requestOpen === requestGoToLineDialogOpen) {
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
  <IndentationPicker bind:open={indentPickerOpen} bind:indentSelection content={value} />
  <TransformationsPalette executeAction={executeTransformation} />

  <div class="flex flex-1 min-h-0 min-w-0 relative">
    <EditorLoader
      visible={editorState.loader.visible}
      message={editorState.loader.message}
      subMessage={editorState.loader.subMessage}
      progress={editorState.loader.progress}
      onCancel={transformationCancelFn ?? undefined}
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
                indentConfig={effectiveIndentConfig}
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
                  indentConfig={effectiveIndentConfig}
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
              <MarkdownPreview
                content={value}
                {editorView}
                documentId={editorState.currentDocumentId}
                documentGeneration={editorState.currentDocumentGeneration}
              />
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
              indentConfig={effectiveIndentConfig}
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
    indentConfig={effectiveIndentConfig}
    onGoToLine={openGoToLinePanel}
    onOpenIndentPicker={openIndentPicker}
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
