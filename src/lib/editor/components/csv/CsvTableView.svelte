<script lang="ts">
  import {
    createTable,
    getCoreRowModel,
    type ColumnDef,
  } from "@tanstack/svelte-table";
  import { untrack } from "svelte";
  import { useScrollVirtualizer } from "./useScrollVirtualizer.svelte";
  import {
    editorState,
    hideEditorLoader,
    startLoaderTicker,
    stopLoaderTicker,
    completeEditorLoader,
  } from "$lib/state/editor.svelte";
  import { useCsvEditorState } from "./useCsvEditorState.svelte";
  import { hotkey } from "$lib/hotkeys";
  import CsvTableHeader from "./CsvTableHeader.svelte";
  import CsvTableBody from "./CsvTableBody.svelte";
  import CsvContextMenu from "./CsvContextMenu.svelte";
  import type {
    CsvReplayStep,
    CsvRowWindow,
    CsvTableController,
    CsvTableFlushResult,
    CsvTableSnapshot,
    CsvWorkerRequest,
    CsvWorkerResponse,
  } from "./csvTableProtocol";

  type Updater<T> = T | ((old: T) => T);

  type ColumnSizingState = Record<string, number>;

  type ColumnSizingInfoState = {
    columnSizingStart: [string, number][];
    deltaOffset: number | null;
    deltaPercentage: number | null;
    isResizingColumn: string | false;
    startOffset: number | null;
    startSize: number | null;
  };

  type PendingRequest = {
    resolve: (value: CsvWorkerResponse) => void;
    reject: (error: Error) => void;
  };

  type CsvWorkerRequestPayload = CsvWorkerRequest extends infer T
    ? T extends { requestId: number }
      ? Omit<T, "requestId">
      : never
    : never;

  const EMPTY_SNAPSHOT: CsvTableSnapshot = {
    headers: [],
    rowCount: 0,
    delimiter: ",",
    errors: [],
    version: 0,
  };

  const EMPTY_ROW_WINDOW: CsvRowWindow = {
    start: 0,
    rows: [],
    version: 0,
  };

  const VIEWPORT_PREFETCH_ROWS = 80;

  function applyUpdater<T>(updater: Updater<T>, current: T): T {
    if (typeof updater === "function") {
      const updaterFn = updater as (old: T) => T;
      return updaterFn(current);
    }

    return updater;
  }

  function reorderColumnSizing(
    sizing: ColumnSizingState,
    start: number,
    end: number,
    target: number,
  ): ColumnSizingState {
    const entries = snapshot.headers.map((_, index) => sizing[`col_${index}`] ?? null);
    const movedEntries = entries.slice(start, end + 1);
    const remainingEntries = [
      ...entries.slice(0, start),
      ...entries.slice(end + 1),
    ];
    remainingEntries.splice(target, 0, ...movedEntries);

    return remainingEntries.reduce<ColumnSizingState>((nextSizing, size, index) => {
      if (size !== null) {
        nextSizing[`col_${index}`] = size;
      }
      return nextSizing;
    }, {});
  }

  let {
    content = $bindable(""),
    tableInfo = $bindable({ rows: 0, cols: 0, delimiter: "", errors: 0 }),
  } = $props();

  let snapshot = $state.raw<CsvTableSnapshot>(EMPTY_SNAPSHOT);
  let rowWindow = $state.raw<CsvRowWindow>(EMPTY_ROW_WINDOW);
  let initialLoading = $state(true);
  let refreshing = $state(false);

  let lastSyncedContent = $state(content);
  let tableWorker: Worker | undefined;
  let nextRequestId = 0;
  let pendingRequests = new Map<number, PendingRequest>();
  let latestRowWindowToken = 0;
  let replayBaseText = "";
  let replayUndoStack: CsvReplayStep[] = [];
  let replayRedoStack: CsvReplayStep[] = [];

  let tableContainerRef = $state<HTMLDivElement | undefined>(undefined);
  let contextMenu = $state<{
    openMenu: (
      x: number,
      y: number,
      options?: { mode?: "selection" | "insert-column" },
    ) => void;
  } | null>(null);
  let wrapperRef = $state<HTMLDivElement | undefined>(undefined);

  function formatRowCount(count: number): string {
    if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M rows…`;
    if (count >= 1_000) return `${(count / 1_000).toFixed(0)}K rows…`;
    return `${count} rows…`;
  }

  function resetPendingRequests(message: string): void {
    const error = new Error(message);
    for (const pending of pendingRequests.values()) {
      pending.reject(error);
    }
    pendingRequests.clear();
  }

  function createTableWorker() {
    if (tableWorker) {
      tableWorker.terminate();
      resetPendingRequests("CSV table worker restarted");
    }

    tableWorker = new Worker(
      new URL("../../workers/csvTable.worker.ts", import.meta.url),
      { type: "module" },
    );

    tableWorker.onmessage = (event: MessageEvent<CsvWorkerResponse>) => {
      const message = event.data;

      if (message.type === "initialize-progress") {
        if (initialLoading) {
          editorState.loader.subMessage = formatRowCount(message.parsedRows);
        }
        return;
      }

      const pending = pendingRequests.get(message.requestId);
      if (!pending) return;

      if (message.type === "error") {
        pendingRequests.delete(message.requestId);
        pending.reject(new Error(message.error));
        return;
      }

      pendingRequests.delete(message.requestId);
      pending.resolve(message);
    };
  }

  function sendRequest(
    request: CsvWorkerRequestPayload,
  ): Promise<CsvWorkerResponse> {
    if (!tableWorker) {
      return Promise.reject(new Error("CSV table worker is not ready"));
    }

    const requestId = ++nextRequestId;
    const payload = { ...request, requestId } as CsvWorkerRequest;

    return new Promise((resolve, reject) => {
      pendingRequests.set(requestId, { resolve, reject });
      tableWorker!.postMessage(payload);
    });
  }

  function resetReplayState(baseText: string): void {
    replayBaseText = baseText;
    lastSyncedContent = baseText;
    replayUndoStack = [];
    replayRedoStack = [];
  }

  function recordReplayStep(previousText: string, nextText: string, userEvent: string): void {
    if (previousText === nextText) {
      return;
    }

    replayUndoStack.push({ text: nextText, userEvent });
    replayRedoStack = [];
  }

  function applyReplayUndo(): void {
    const step = replayUndoStack.pop();
    if (!step) {
      return;
    }

    replayRedoStack.push(step);
  }

  function applyReplayRedo(): void {
    const step = replayRedoStack.pop();
    if (!step) {
      return;
    }

    replayUndoStack.push(step);
  }

  export async function flushToTextHistory(): Promise<CsvTableFlushResult> {
    const response = await sendRequest({ type: "flush-text" });
    if (response.type !== "flushed-text") {
      throw new Error("Unexpected CSV flush response");
    }

    snapshot = { ...snapshot, version: response.version };
    lastSyncedContent = response.text;
    content = response.text;
    return {
      baseText: replayBaseText,
      text: response.text,
      replaySteps: replayUndoStack.map((step) => ({
        text: step.text,
        userEvent: step.userEvent,
      })),
      version: response.version,
    };
  }

  function getVisibleRow(index: number): string[] | undefined {
    const offset = index - rowWindow.start;
    if (offset < 0 || offset >= rowWindow.rows.length) {
      return undefined;
    }
    return rowWindow.rows[offset];
  }

  async function refreshViewportRows(force = false): Promise<void> {
    if (snapshot.rowCount === 0 || snapshot.headers.length === 0) {
      rowWindow = { ...EMPTY_ROW_WINDOW, version: snapshot.version };
      return;
    }

    const items = virtualizer.virtualItems;
    let start = 0;
    let end = Math.min(snapshot.rowCount - 1, VIEWPORT_PREFETCH_ROWS);

    if (items.length > 0) {
      start = Math.max(0, items[0].index - VIEWPORT_PREFETCH_ROWS);
      end = Math.min(
        snapshot.rowCount - 1,
        items[items.length - 1].index + VIEWPORT_PREFETCH_ROWS,
      );
    }

    const currentEnd = rowWindow.start + rowWindow.rows.length - 1;
    if (
      !force &&
      rowWindow.version === snapshot.version &&
      start >= rowWindow.start &&
      end <= currentEnd
    ) {
      return;
    }

    const token = ++latestRowWindowToken;
    const response = await sendRequest({ type: "get-rows", start, end });
    if (response.type !== "rows") return;
    if (token !== latestRowWindowToken) return;
    if (response.window.version !== snapshot.version) return;
    rowWindow = response.window;
  }

  async function applyMutationResponse(
    response: CsvWorkerResponse,
    userEvent: string,
    mode: "forward" | "undo" | "redo",
  ): Promise<boolean> {
    if (response.type !== "mutation-applied") {
      return false;
    }

    if (!response.applied) {
      return false;
    }

    const previousText = lastSyncedContent;
    snapshot = response.snapshot;
    lastSyncedContent = response.text;
    content = response.text;

    if (mode === "forward") {
      recordReplayStep(previousText, response.text, userEvent);
    } else if (mode === "undo") {
      applyReplayUndo();
    } else {
      applyReplayRedo();
    }

    rowWindow = { ...EMPTY_ROW_WINDOW, version: snapshot.version };
    await refreshViewportRows(true);
    return true;
  }

  const controller: CsvTableController = {
    getSnapshot: () => snapshot,
    getCachedCellValue(rowIndex: number, colIndex: number) {
      if (rowIndex === -1) {
        return snapshot.headers[colIndex] ?? "";
      }
      return getVisibleRow(rowIndex)?.[colIndex] ?? "";
    },
    async fetchCellValue(rowIndex: number, colIndex: number) {
      if (rowIndex === -1) {
        return snapshot.headers[colIndex] ?? "";
      }
      const response = await sendRequest({ type: "get-cell", rowIndex, colIndex });
      return response.type === "cell" ? response.value : "";
    },
    async runMutation(mutation, userEvent) {
      const response = await sendRequest({ type: "mutate", mutation });
      return applyMutationResponse(response, userEvent, "forward");
    },
    async undo() {
      const response = await sendRequest({ type: "undo" });
      return applyMutationResponse(response, "undo.table", "undo");
    },
    async redo() {
      const response = await sendRequest({ type: "redo" });
      return applyMutationResponse(response, "redo.table", "redo");
    },
  };

  async function initializeTable(text: string, options?: { initial?: boolean }) {
    const isInitial = options?.initial ?? false;

    if (isInitial) {
      initialLoading = true;
      refreshing = false;
      startLoaderTicker("Parsing CSV…", "Starting…", {
        ceiling: 85,
        factor: 0.05,
        minStep: 0.2,
        interval: 100,
      });
    } else {
      refreshing = true;
    }

    createTableWorker();

    try {
      const response = await sendRequest({ type: "initialize", text });
      if (response.type !== "initialized") {
        throw new Error("Unexpected CSV initialization response");
      }

      resetReplayState(text);
      snapshot = response.snapshot;
      rowWindow = { ...EMPTY_ROW_WINDOW, version: snapshot.version };
      latestRowWindowToken += 1;
      await refreshViewportRows(true);

      if (isInitial) {
        initialLoading = false;
        completeEditorLoader(
          "Building table…",
          `${snapshot.rowCount.toLocaleString()} rows`,
        );

        if (
          snapshot.rowCount > 0 &&
          snapshot.headers.length > 0 &&
          !csvEditorState.focusedCell
        ) {
          csvEditorState.focusedCell = { rowIndex: 0, colIndex: 0 };
          csvEditorState.navigateAndFocus();
        }
      }

      refreshing = false;
    } catch (error) {
      if (isInitial) {
        stopLoaderTicker();
        hideEditorLoader();
        initialLoading = false;
      }
      refreshing = false;
      console.error("CSV table worker initialization error:", error);
    }
  }

  let prevHeaderKey = "";
  let stableColumns: ColumnDef<string[], string>[] = [];

  let columns = $derived.by(() => {
    const headerKey = `${snapshot.headers.length}\0${snapshot.headers.join("\0")}`;
    if (headerKey === prevHeaderKey) return stableColumns;
    prevHeaderKey = headerKey;
    stableColumns = snapshot.headers.map(
      (header, index): ColumnDef<string[], string> => ({
        id: `col_${index}`,
        accessorFn: (row: string[]) => row[index] ?? "",
        header: header || `Column ${index + 1}`,
        size: 150,
        minSize: 60,
      }),
    );
    return stableColumns;
  });

  let visibleRowCount = $derived(
    snapshot.headers.length === 0 ? 0 : snapshot.rowCount,
  );

  const csvEditorState = useCsvEditorState(
    controller,
    () => columns,
    (index, options) => virtualizer.scrollToIndex(index, options),
    (start, end, target) => {
      columnSizing = reorderColumnSizing(columnSizing, start, end, target);
    },
    () => {
      wrapperRef?.focus();
    },
  );

  $effect(() => {
    const undoFn = () => csvEditorState.handleUndo();
    const redoFn = () => csvEditorState.handleRedo();
    editorState.csv.undo = undoFn;
    editorState.csv.redo = redoFn;

    return () => {
      if (editorState.csv.undo === undoFn) editorState.csv.undo = undefined;
      if (editorState.csv.redo === redoFn) editorState.csv.redo = undefined;
    };
  });

  let columnSizing = $state<ColumnSizingState>({});
  let columnSizingInfo = $state<ColumnSizingInfoState>({
    startOffset: null,
    startSize: null,
    deltaOffset: null,
    deltaPercentage: null,
    isResizingColumn: false,
    columnSizingStart: [],
  });

  const table = createTable({
    get data() {
      return [] as string[][];
    },
    get columns() {
      return columns;
    },
    state: {
      get columnSizing() {
        return columnSizing;
      },
      get columnSizingInfo() {
        return columnSizingInfo;
      },
    },
    onColumnSizingChange: (updater) => {
      columnSizing = applyUpdater(updater, columnSizing);
    },
    onColumnSizingInfoChange: (updater) => {
      columnSizingInfo = applyUpdater(updater, columnSizingInfo);
    },
    onStateChange: () => {},
    renderFallbackValue: null,
    getCoreRowModel: getCoreRowModel(),
    enableColumnResizing: true,
    columnResizeMode: "onChange" as const,
  });

  const virtualizer = useScrollVirtualizer({
    count: () => visibleRowCount,
    getScrollElement: () => tableContainerRef,
    estimateSize: () => 32,
    overscan: 20,
  });

  let indexColWidth = $derived(
    Math.max(50, 24 + String(snapshot.rowCount).length * 8),
  );

  let delimiterLabel = $derived.by(() => {
    switch (snapshot.delimiter) {
      case ",":
        return "Comma";
      case "\t":
        return "Tab";
      case ";":
        return "Semicolon";
      case "|":
        return "Pipe";
      case ":":
        return "Colon";
      case "~":
        return "Tilde";
      default:
        return snapshot.delimiter;
    }
  });

  $effect(() => {
    tableInfo = {
      rows: snapshot.rowCount,
      cols: snapshot.headers.length,
      delimiter: delimiterLabel,
      errors: snapshot.errors.length,
    };
  });

  $effect(() => {
    void initializeTable(untrack(() => content), { initial: true });

    return () => {
      stopLoaderTicker();
      hideEditorLoader();
      resetPendingRequests("CSV table worker disposed");
      tableWorker?.terminate();
    };
  });

  $effect(() => {
    if (content !== untrack(() => lastSyncedContent)) {
      lastSyncedContent = content;
      void initializeTable(content, { initial: false });
    }
  });

  $effect(() => {
    if (initialLoading) return;
    snapshot.version;
    virtualizer.virtualItems;
    void refreshViewportRows();
  });
</script>

<svelte:window
  onmouseup={() => {
    if (columnSizingInfo?.isResizingColumn) {
      table.resetColumnSizing();
    }
  }}
  onblur={() => {
    if (columnSizingInfo?.isResizingColumn) {
      table.resetColumnSizing();
    }
  }}
  onmouseleave={() => {
    if (columnSizingInfo?.isResizingColumn) {
      table.resetColumnSizing();
    }
  }}
/>

{#if !initialLoading}
  <CsvContextMenu
    bind:this={contextMenu}
    editorState={csvEditorState}
    container={wrapperRef}
  />
  <div
    class="csv-table-wrapper"
    class:csv-table-refreshing={refreshing}
    bind:this={wrapperRef}
    tabindex="0"
    role="grid"
    use:hotkey={csvEditorState.cellHotkeys}
    onkeydown={(e) => csvEditorState.handleCellKeydown(e)}
    onpointerdown={(e) => {
      if ((e.target as HTMLElement).tagName.toLowerCase() === "input") return;
      const cell = (e.target as HTMLElement).closest("[data-row][data-col]");
      if (!cell || e.button === 2) return;
      const rowIndex = parseInt(cell.getAttribute("data-row")!, 10);
      const colIndex = parseInt(cell.getAttribute("data-col")!, 10);
      csvEditorState.isSelecting = true;
      if (rowIndex === -1 && colIndex === -1) return;
      let startR = rowIndex;
      let endR = rowIndex;
      let startC = colIndex;
      let endC = colIndex;
      if (colIndex === -1) {
        startC = 0;
        endC = snapshot.headers.length - 1;
      }
      if (rowIndex === -1) {
        startR = 0;
        endR = snapshot.rowCount - 1;
      }

      if (e.shiftKey && csvEditorState.focusedCell) {
        const fr = csvEditorState.focusedCell.rowIndex;
        const fc = csvEditorState.focusedCell.colIndex;
        if (colIndex === -1) {
          startR = Math.min(fr, rowIndex);
          endR = Math.max(fr, rowIndex);
          startC = 0;
          endC = snapshot.headers.length - 1;
        } else if (rowIndex === -1) {
          startR = 0;
          endR = snapshot.rowCount - 1;
          startC = Math.min(fc, colIndex);
          endC = Math.max(fc, colIndex);
        } else {
          startR = Math.min(fr, rowIndex);
          endR = Math.max(fr, rowIndex);
          startC = Math.min(fc, colIndex);
          endC = Math.max(fc, colIndex);
        }
      } else {
        if (rowIndex >= 0 && colIndex >= 0)
          csvEditorState.focusedCell = { rowIndex, colIndex };
        else if (rowIndex >= 0)
          csvEditorState.focusedCell = { rowIndex, colIndex: 0 };
        else if (colIndex >= 0)
          csvEditorState.focusedCell = { rowIndex: -1, colIndex };
        csvEditorState.navigateAndFocus();
      }
      csvEditorState.selectionBlock = {
        startRow: startR,
        endRow: endR,
        startCol: startC,
        endCol: endC,
      };
    }}
    onpointerover={(e) => {
      if ((e.target as HTMLElement).tagName.toLowerCase() === "input") return;
      if (
        !csvEditorState.isSelecting ||
        !csvEditorState.selectionBlock ||
        !csvEditorState.focusedCell
      )
        return;
      const cell = (e.target as HTMLElement).closest("[data-row][data-col]");
      if (!cell) return;
      const rowIndex = parseInt(cell.getAttribute("data-row")!, 10);
      const colIndex = parseInt(cell.getAttribute("data-col")!, 10);
      if (rowIndex === -1 && colIndex === -1) return;
      const fr = csvEditorState.focusedCell.rowIndex;
      const fc = csvEditorState.focusedCell.colIndex;
      let startR = Math.min(Math.max(0, fr), Math.max(0, rowIndex));
      let endR = Math.max(Math.max(0, fr), rowIndex);
      let startC = Math.min(Math.max(0, fc), Math.max(0, colIndex));
      let endC = Math.max(Math.max(0, fc), colIndex);

      if (colIndex === -1 || fc === -1) {
        startC = 0;
        endC = snapshot.headers.length - 1;
        startR = Math.min(fr >= 0 ? fr : rowIndex, rowIndex >= 0 ? rowIndex : fr);
        endR = Math.max(fr >= 0 ? fr : rowIndex, rowIndex >= 0 ? rowIndex : fr);
      }
      if (rowIndex === -1 || fr === -1) {
        startR = 0;
        endR = snapshot.rowCount - 1;
        startC = Math.min(fc >= 0 ? fc : colIndex, colIndex >= 0 ? colIndex : fc);
        endC = Math.max(fc >= 0 ? fc : colIndex, colIndex >= 0 ? colIndex : fc);
      }
      csvEditorState.selectionBlock = {
        startRow: startR,
        endRow: endR,
        startCol: startC,
        endCol: endC,
      };
    }}
    onpointerup={() => {
      csvEditorState.isSelecting = false;
    }}
    oncontextmenu={(e) => {
      e.preventDefault();
      const cell = (e.target as HTMLElement).closest("[data-row][data-col]");
      if (cell) {
        const rowIndex = parseInt(cell.getAttribute("data-row")!, 10);
        const colIndex = parseInt(cell.getAttribute("data-col")!, 10);

        const numRows = snapshot.rowCount;
        const numCols = columns.length;

        if (colIndex === -1 && rowIndex === -1) {
          contextMenu?.openMenu(e.clientX, e.clientY, { mode: "insert-column" });
          return;
        }

        let inSelection = false;
        if (csvEditorState.selectionBlock) {
          const sb = csvEditorState.selectionBlock;
          if (rowIndex === -1) {
            if (
              colIndex >= sb.startCol &&
              colIndex <= sb.endCol &&
              sb.startRow === 0 &&
              sb.endRow >= numRows - 1
            ) {
              inSelection = true;
            }
          } else if (colIndex === -1) {
            if (
              rowIndex >= sb.startRow &&
              rowIndex <= sb.endRow &&
              sb.startCol === 0 &&
              sb.endCol >= numCols - 1
            ) {
              inSelection = true;
            }
          } else {
            if (
              rowIndex >= sb.startRow &&
              rowIndex <= sb.endRow &&
              colIndex >= sb.startCol &&
              colIndex <= sb.endCol
            ) {
              inSelection = true;
            }
          }
        }

        if (!inSelection) {
          if (rowIndex === -1) {
            csvEditorState.selectionBlock = {
              startRow: 0,
              endRow: numRows - 1,
              startCol: colIndex,
              endCol: colIndex,
            };
          } else if (colIndex === -1) {
            csvEditorState.selectionBlock = {
              startRow: rowIndex,
              endRow: rowIndex,
              startCol: 0,
              endCol: numCols - 1,
            };
          } else {
            csvEditorState.selectionBlock = {
              startRow: rowIndex,
              endRow: rowIndex,
              startCol: colIndex,
              endCol: colIndex,
            };
          }
          if (rowIndex >= 0 && colIndex >= 0) {
            csvEditorState.focusedCell = { rowIndex, colIndex };
          }
        }

        const sb = csvEditorState.selectionBlock;
        if (sb) {
          const isRowSelection = sb.startCol === 0 && sb.endCol >= numCols - 1;
          const isColSelection = sb.startRow === 0 && sb.endRow >= numRows - 1;
          if (isRowSelection || isColSelection) {
            contextMenu?.openMenu(e.clientX, e.clientY);
          }
        }
      }
    }}
  >
    <div class="csv-table-container" bind:this={tableContainerRef}>
      {#if refreshing}
        <div class="csv-refresh-overlay" aria-hidden="true">
          Refreshing from text changes...
        </div>
      {/if}
      <div
        style="height: {virtualizer.totalSize}px; width: 100%; min-width: max-content; padding-right: 200px; position: relative;"
      >
        <CsvTableHeader
          {table}
          {indexColWidth}
          editorState={csvEditorState}
          totalRows={snapshot.rowCount}
        />
        <CsvTableBody
          {table}
          {virtualizer}
          {indexColWidth}
          editorState={csvEditorState}
          getRow={getVisibleRow}
        />
      </div>
    </div>
  </div>
{/if}

<style>
  .csv-table-wrapper {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    min-width: 0;
    width: 100%;
    overflow: hidden;
    background: var(--background);
    color: var(--foreground);
    outline: none;
  }

  .csv-table-wrapper.csv-table-refreshing {
    position: relative;
  }

  .csv-table-container {
    flex: 1;
    min-height: 0;
    min-width: 0;
    overflow: auto;
    overscroll-behavior: none;
    position: relative;
  }

  .csv-refresh-overlay {
    position: sticky;
    top: 0;
    z-index: 30;
    margin: 8px 12px 0 auto;
    width: fit-content;
    max-width: calc(100% - 24px);
    padding: 4px 8px;
    border: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
    border-radius: 999px;
    background: color-mix(in srgb, var(--background) 92%, transparent);
    color: var(--muted-foreground);
    font-size: 11px;
    backdrop-filter: blur(6px);
    pointer-events: none;
  }
</style>