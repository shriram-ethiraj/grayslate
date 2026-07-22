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
  import { invoke, invokeText } from "$lib/ipc";
  import { Channel } from "@tauri-apps/api/core";
  import { copyCsvSessionToClipboard } from "$lib/clipboard";
  import type {
    CsvMirrorTextUpdate,
    CsvMutationRequest,
    CsvRowWindow,
    CsvTableController,
    CsvTableFlushResult,
    CsvTableSnapshot,
    CsvMutationResponse,
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

  const EMPTY_SNAPSHOT: CsvTableSnapshot = {
    headers: [],
    rowCount: 0,
    delimiter: ",",
    errors: [],
    version: 0,
    liveMirrorEnabled: false,
  };

  const EMPTY_ROW_WINDOW: CsvRowWindow = {
    start: 0,
    rows: [],
    version: 0,
  };

  const VIEWPORT_PREFETCH_ROWS = 80;
  const DEFAULT_CSV_ROW_FONT_SIZE = 13;
  const DEFAULT_CSV_HEADER_FONT_SIZE = 12;
  const DEFAULT_CSV_INDEX_FONT_SIZE = 11;
  const DEFAULT_CSV_ROW_HEIGHT = 32;
  const DEFAULT_CSV_HEADER_HEIGHT = 34;
  const REFERENCE_EDITOR_FONT_SIZE = 14;

  function scaleFromEditorFont(baseSize: number): number {
    return Math.max(
      8,
      Math.round((editorState.fontSize * baseSize) / REFERENCE_EDITOR_FONT_SIZE),
    );
  }

  function applyUpdater<T>(updater: Updater<T>, current: T): T {
    if (typeof updater === "function") {
      const updaterFn = updater as (old: T) => T;
      return updaterFn(current);
    }

    return updater;
  }

  function reorderColumnSizing(
    sizing: ColumnSizingState,
    colCount: number,
    start: number,
    end: number,
    target: number,
  ): ColumnSizingState {
    const entries = Array.from({ length: colCount }, (_, index) => sizing[`col_${index}`] ?? null);
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
    tableInfo = $bindable({
      rows: 0,
      cols: 0,
      delimiter: "",
      errors: 0,
      liveMirrorEnabled: false,
    }),
    onMirrorReset,
    onMirrorUpdate,
  } = $props();

  let snapshot = $state.raw<CsvTableSnapshot>(EMPTY_SNAPSHOT);
  let rowWindow = $state.raw<CsvRowWindow>(EMPTY_ROW_WINDOW);
  let initialLoading = $state(true);
  let refreshing = $state(false);

  let lastSyncedContent = $state(content);
  let latestRowWindowToken = 0;
  let pendingMutation: Promise<boolean> | undefined;

  let tableContainerRef = $state<HTMLDivElement | undefined>(undefined);
  let contextMenu = $state<{
    openMenu: (
      x: number,
      y: number,
      options?: { mode?: "selection" | "insert-column" },
    ) => void;
  } | null>(null);
  let wrapperRef = $state<HTMLDivElement | undefined>(undefined);

  function resetColumnSizingInfoState(): ColumnSizingInfoState {
    return {
      startOffset: null,
      startSize: null,
      deltaOffset: null,
      deltaPercentage: null,
      isResizingColumn: false,
      columnSizingStart: [],
    };
  }

  function formatRowCount(count: number): string {
    if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M rows…`;
    if (count >= 1_000) return `${(count / 1_000).toFixed(0)}K rows…`;
    return `${count} rows…`;
  }

  function disposeSession(): void {
    invoke("csv_dispose").catch(() => {});
  }

  function releaseLargeTableState(): void {
    latestRowWindowToken += 1;
    snapshot = EMPTY_SNAPSHOT;
    rowWindow = EMPTY_ROW_WINDOW;
    pendingMutation = undefined;
    lastSyncedContent = "";
    prevHeaderKey = "";
    stableColumns = [];
    columnSizing = {};
    columnSizingInfo = resetColumnSizingInfoState();
    tableContainerRef = undefined;
    wrapperRef = undefined;
    contextMenu = null;
    tableInfo = {
      rows: 0,
      cols: 0,
      delimiter: "",
      errors: 0,
      liveMirrorEnabled: false,
    };
    csvEditorState.dispose();
  }

  function resetReplayState(baseText: string): void {
    lastSyncedContent = baseText;
    onMirrorReset?.(baseText);
  }

  export async function flushToTextHistory(): Promise<CsvTableFlushResult> {
    const text = await invokeText("csv_flush_text");
    const version = snapshot.version;
    snapshot = { ...snapshot, version };
    lastSyncedContent = text;
    content = text;
    return { text, version };
  }

  async function copyAllCsv(): Promise<boolean> {
    try {
      await pendingMutation;
    } catch {
      return false;
    }
    const copied = await copyCsvSessionToClipboard(content.length);
    if (copied) {
      wrapperRef?.focus();
    }
    return copied;
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
    const window = await invoke<CsvRowWindow>("csv_get_rows", { start, end });
    if (token !== latestRowWindowToken) return;
    if (window.version !== snapshot.version) return;
    rowWindow = window;
  }

  async function applyMutationResponse(
    response: CsvMutationResponse,
  ): Promise<boolean> {
    if (!response.applied) {
      return false;
    }

    // Forward mirror text to EditorWrapper before updating snapshot,
    // so the mirror queue receives the update before viewport refresh.
    if (response.mirrorText != null && response.mirrorUserEvent) {
      const update: CsvMirrorTextUpdate = {
        text: response.mirrorText,
        userEvent: response.mirrorUserEvent,
        version: response.snapshot.version,
      };
      onMirrorUpdate?.(update);
    }

    snapshot = response.snapshot;
    // Do NOT clear rowWindow here — the stale rows stay visible while the
    // new viewport window is fetched via IPC, preventing a flash to empty.
    await refreshViewportRows(true);
    return true;
  }

  function trackMutation(operation: Promise<boolean>): Promise<boolean> {
    const tracked = operation.finally(() => {
      if (pendingMutation === tracked) {
        pendingMutation = undefined;
      }
    });
    pendingMutation = tracked;
    return tracked;
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
      return invoke<string>("csv_get_cell", { rowIndex, colIndex });
    },
    runMutation(mutation, userEvent) {
      return trackMutation(
        invoke<CsvMutationResponse>("csv_mutate", { mutation, userEvent }).then(
          applyMutationResponse,
        ),
      );
    },
    undo() {
      return trackMutation(
        invoke<CsvMutationResponse>("csv_undo").then(applyMutationResponse),
      );
    },
    redo() {
      return trackMutation(
        invoke<CsvMutationResponse>("csv_redo").then(applyMutationResponse),
      );
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

    // Dispose any previous session before creating a new one.
    disposeSession();

    try {
      const progressChannel = new Channel<{ type: string; parsedRows: number }>();
      progressChannel.onmessage = (event) => {
        if (event.type === "progress" && initialLoading) {
          editorState.loader.subMessage = formatRowCount(event.parsedRows);
        }
      };

      const initSnapshot = await invoke<CsvTableSnapshot>("csv_initialize", {
        text,
        onEvent: progressChannel,
      });

      resetReplayState(text);
      snapshot = initSnapshot;
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
      console.error("CSV table initialization error:", error);
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
      columnSizing = reorderColumnSizing(columnSizing, snapshot.headers.length, start, end, target);
    },
    () => {
      wrapperRef?.focus();
    },
    copyAllCsv,
  );

  $effect(() => {
    const copyFn = () => csvEditorState.handleCopy();
    const undoFn = () => csvEditorState.handleUndo();
    const redoFn = () => csvEditorState.handleRedo();
    editorState.csv.copy = copyFn;
    editorState.csv.undo = undoFn;
    editorState.csv.redo = redoFn;

    return () => {
      if (editorState.csv.copy === copyFn) editorState.csv.copy = undefined;
      if (editorState.csv.undo === undoFn) editorState.csv.undo = undefined;
      if (editorState.csv.redo === redoFn) editorState.csv.redo = undefined;
    };
  });

  let columnSizing = $state<ColumnSizingState>({});
  let columnSizingInfo = $state<ColumnSizingInfoState>(resetColumnSizingInfoState());

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
    estimateSize: () => csvRowHeight,
    headerHeight: () => csvHeaderHeight,
    overscan: 20,
  });

  let csvRowFontSize = $derived(scaleFromEditorFont(DEFAULT_CSV_ROW_FONT_SIZE));
  let csvHeaderFontSize = $derived(scaleFromEditorFont(DEFAULT_CSV_HEADER_FONT_SIZE));
  let csvIndexFontSize = $derived(scaleFromEditorFont(DEFAULT_CSV_INDEX_FONT_SIZE));
  let csvRowHeight = $derived(
    Math.max(
      DEFAULT_CSV_ROW_HEIGHT,
      Math.ceil(csvRowFontSize * 1.5 + 12),
    ),
  );
  let csvHeaderHeight = $derived(
    Math.max(
      DEFAULT_CSV_HEADER_HEIGHT,
      Math.ceil(csvHeaderFontSize * 1.5 + 16),
    ),
  );
  let csvOverlayFontSize = $derived(Math.max(10, scaleFromEditorFont(DEFAULT_CSV_INDEX_FONT_SIZE)));

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
      liveMirrorEnabled: snapshot.liveMirrorEnabled,
    };
  });

  $effect(() => {
    void initializeTable(untrack(() => content), { initial: true });

    return () => {
      stopLoaderTicker();
      hideEditorLoader();
      disposeSession();
      releaseLargeTableState();
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
    data-testid="csv-table"
    class="csv-table-wrapper"
    class:csv-table-refreshing={refreshing}
    style={`--csv-row-font-size: ${csvRowFontSize}px; --csv-header-font-size: ${csvHeaderFontSize}px; --csv-index-font-size: ${csvIndexFontSize}px; --csv-row-height: ${csvRowHeight}px; --csv-header-height: ${csvHeaderHeight}px; --csv-overlay-font-size: ${csvOverlayFontSize}px;`}
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

        if (
          csvEditorState.selectionBlock &&
          (csvEditorState.isRowSelection() || csvEditorState.isColumnSelection())
        ) {
          contextMenu?.openMenu(e.clientX, e.clientY);
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
    font-size: var(--csv-overlay-font-size, 11px);
    backdrop-filter: blur(6px);
    pointer-events: none;
  }
</style>
