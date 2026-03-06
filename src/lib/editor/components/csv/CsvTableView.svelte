<script lang="ts">
  import {
    createTable,
    getCoreRowModel,
    type ColumnDef,
  } from "@tanstack/svelte-table";
  import { useScrollVirtualizer } from "./useScrollVirtualizer.svelte";
  import {
    editorState,
    hideEditorLoader,
    startLoaderTicker,
    stopLoaderTicker,
    completeEditorLoader,
  } from "$lib/state/editor.svelte";
  import { untrack } from "svelte";
  import { useCsvHistory } from "./useCsvHistory.svelte";
  import { useCsvEditorState } from "./useCsvEditorState.svelte";
  import { hotkey } from "$lib/hotkeys";
  import CsvTableHeader from "./CsvTableHeader.svelte";
  import CsvTableBody from "./CsvTableBody.svelte";
  import CsvContextMenu from "./CsvContextMenu.svelte";

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

  function applyUpdater<T>(updater: Updater<T>, current: T): T {
    if (typeof updater === "function") {
      const updaterFn = updater as (old: T) => T;
      return updaterFn(current);
    }

    return updater;
  }

  let {
    content = $bindable(""),
    tableInfo = $bindable({ rows: 0, cols: 0, delimiter: "", errors: 0 }),
    onMirrorTextChange = undefined,
  } = $props();

  // 1. Parsed data — starts empty, filled by worker
  let parsed = $state.raw<{
    headers: string[];
    rows: string[][];
    delimiter: string;
    errors: string[];
  }>({ headers: [], rows: [], delimiter: ",", errors: [] });

  let initialLoading = $state(true);
  let refreshing = $state(false);
  let pendingRows: string[][] = [];

  // Track the last content we synced from, to detect external changes
  let lastSyncedContent = $state(content);
  let parseWorker: Worker | undefined;
  let activeParseRequestId = 0;
  let nextParseRequestId = 0;

  /** Format a row count for display: 7 000 000 → "7.0M rows…" */
  function formatRowCount(count: number): string {
    if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M rows…`;
    if (count >= 1_000) return `${(count / 1_000).toFixed(0)}K rows…`;
    return `${count} rows…`;
  }

  function createParseWorker() {
    parseWorker?.terminate();
    parseWorker = new Worker(
      new URL("../../workers/csvParser.worker.ts", import.meta.url),
      { type: "module" },
    );

    parseWorker.onmessage = (e) => {
      const msg = e.data;
      if (msg.requestId !== activeParseRequestId) {
        return;
      }

      switch (msg.type) {
        case "parsed-chunk": {
          const chunk: string[][] = msg.chunk;
          for (let i = 0; i < chunk.length; i++) {
            pendingRows.push(chunk[i]);
          }

          if (initialLoading) {
            editorState.loader.subMessage = formatRowCount(pendingRows.length);
          }
          break;
        }
        case "parsed-complete": {
          const rowCount = pendingRows.length;
          parsed = {
            headers: msg.headers,
            rows: pendingRows,
            delimiter: msg.delimiter,
            errors: msg.errors,
          };
          pendingRows = [];

          if (initialLoading) {
            initialLoading = false;
            completeEditorLoader(
              "Building table…",
              `${rowCount.toLocaleString()} rows`,
            );

            if (rowCount > 0 && !csvEditorState.focusedCell) {
              csvEditorState.focusedCell = { rowIndex: 0, colIndex: 0 };
              csvEditorState.navigateAndFocus();
            }
          }

          refreshing = false;
          break;
        }
        case "error":
          if (initialLoading) {
            stopLoaderTicker();
            hideEditorLoader();
            initialLoading = false;
          }
          console.error("CSV Parse Worker Error:", msg.error);
          pendingRows = [];
          refreshing = false;
          break;
      }
    };
  }

  /** Kick off a parse via the worker. Initial load blocks; later refreshes stay mounted. */
  function startParse(text: string, options?: { initial?: boolean }): void {
    const isInitial = options?.initial ?? false;
    activeParseRequestId = ++nextParseRequestId;
    pendingRows = [];

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

    createParseWorker();
    parseWorker?.postMessage({ text, requestId: activeParseRequestId });
  }

  // Kick off initial parse via worker
  $effect(() => {
    startParse(untrack(() => content), { initial: true });

    return () => {
      stopLoaderTicker();
      parseWorker?.terminate();
      hideEditorLoader();
    };
  });

  // Re-parse when content changes externally (e.g. switching from text mode)
  $effect(() => {
    if (content !== untrack(() => lastSyncedContent)) {
      lastSyncedContent = content;
      startParse(content, { initial: false });
    }
  });

  // 3. TanStack Table Columns (no data passed — we render raw rows directly)

  // Stabilise columns reference — only rebuild when headers actually change
  let prevHeaderKey = "";
  let stableColumns: ColumnDef<string[], string>[] = [];

  let columns = $derived.by(() => {
    const headerKey = parsed.headers.join("\0");
    if (headerKey === prevHeaderKey) return stableColumns;
    prevHeaderKey = headerKey;
    stableColumns = parsed.headers.map(
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

  // 4. Virtualizer Ref
  let tableContainerRef = $state<HTMLDivElement | undefined>(undefined);
  let contextMenu = $state<{ openMenu: (x: number, y: number) => void } | null>(
    null,
  );

  let wrapperRef = $state<HTMLDivElement | undefined>(undefined);
  const history = useCsvHistory();

  // 5. Editor State (Keyboard, Editing, Focus)
  const csvEditorState = useCsvEditorState(
    history,
    () => parsed,
    (p) => {
      parsed = p;
    },
    () => columns,
    (index) => virtualizer.scrollToIndex(index),
    (nextText, userEvent) => {
      content = nextText;
      lastSyncedContent = nextText;
      onMirrorTextChange?.(nextText, userEvent);
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

  // 6. TanStack Table Instance — empty data, only used for column sizing/headers
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

  // 7. Virtualizer Instance
  const virtualizer = useScrollVirtualizer({
    count: () => parsed.rows.length,
    getScrollElement: () => tableContainerRef,
    estimateSize: () => 32,
    overscan: 20,
  });

  // 8. Dynamic Index column width
  // Base 32px + 8px per digit (approx)
  // for 7M rows, 7 digits * 8 = 56 + 32 = 88px
  let indexColWidth = $derived(
    Math.max(50, 24 + String(parsed.rows.length).length * 8),
  );

  // Delimiter display label
  let delimiterLabel = $derived.by(() => {
    switch (parsed.delimiter) {
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
        return parsed.delimiter;
    }
  });

  $effect(() => {
    tableInfo = {
      rows: parsed.rows.length,
      cols: parsed.headers.length,
      delimiter: delimiterLabel,
      errors: parsed.errors.length,
    };
  });
</script>

<!-- Reset dragging if mouse leaves window or loses focus -->
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
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
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
      if (!cell || e.button === 2) return; // Right click handled by contextMenu
      const rowIndex = parseInt(cell.getAttribute("data-row")!, 10);
      const colIndex = parseInt(cell.getAttribute("data-col")!, 10);
      csvEditorState.isSelecting = true;
      if (rowIndex === -1 && colIndex === -1) return; // top left corner
      let startR = rowIndex;
      let endR = rowIndex;
      let startC = colIndex;
      let endC = colIndex;
      if (colIndex === -1) {
        startC = 0;
        endC = parsed.headers.length - 1;
      }
      if (rowIndex === -1) {
        startR = 0;
        endR = parsed.rows.length - 1;
      }

      if (e.shiftKey && csvEditorState.focusedCell) {
        const fr = csvEditorState.focusedCell.rowIndex;
        const fc = csvEditorState.focusedCell.colIndex;
        if (colIndex === -1) {
          startR = Math.min(fr, rowIndex);
          endR = Math.max(fr, rowIndex);
          startC = 0;
          endC = parsed.headers.length - 1;
        } else if (rowIndex === -1) {
          startR = 0;
          endR = parsed.rows.length - 1;
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
          // colIndex >= 0 but rowIndex === -1 means a header cell was clicked
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
        endC = parsed.headers.length - 1;
        startR = Math.min(
          fr >= 0 ? fr : rowIndex,
          rowIndex >= 0 ? rowIndex : fr,
        );
        endR = Math.max(fr >= 0 ? fr : rowIndex, rowIndex >= 0 ? rowIndex : fr);
      }
      if (rowIndex === -1 || fr === -1) {
        startR = 0;
        endR = parsed.rows.length - 1;
        startC = Math.min(
          fc >= 0 ? fc : colIndex,
          colIndex >= 0 ? colIndex : fc,
        );
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
        if (rowIndex === -1 && colIndex === -1) return;

        const numRows = parsed.rows.length;
        const numCols = columns.length;

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
          if (rowIndex >= 0 && colIndex >= 0)
            csvEditorState.focusedCell = { rowIndex, colIndex };
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
    <!-- Table Container -->
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
          totalRows={parsed.rows.length}
        />
        <CsvTableBody
          {table}
          {virtualizer}
          {indexColWidth}
          editorState={csvEditorState}
          rawRows={parsed.rows}
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
