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
  import { debounce } from "lodash-es";
  import { untrack, onDestroy } from "svelte";
  import { useCsvHistory, translateToWorkerOps } from "./useCsvHistory.svelte";
  import { useCsvEditorState } from "./useCsvEditorState.svelte";
  import { hotkey } from "$lib/hotkeys";
  import CsvTableHeader from "./CsvTableHeader.svelte";
  import CsvTableBody from "./CsvTableBody.svelte";
  import CsvContextMenu from "./CsvContextMenu.svelte";

  let {
    content = $bindable(""),
    tableInfo = $bindable({ rows: 0, cols: 0, delimiter: "", errors: 0 }),
  } = $props();

  // 1. Parsed data — starts empty, filled by worker
  let parsed = $state.raw<{
    headers: string[];
    rows: string[][];
    delimiter: string;
    errors: string[];
  }>({ headers: [], rows: [], delimiter: ",", errors: [] });

  let loading = $state(true);
  let pendingRows: string[][] = [];

  // Track the last content we synced from, to detect external changes
  let lastSyncedContent = $state(content);
  let serializeWorker: Worker;
  let parseWorker: Worker;

  /** Format a row count for display: 7 000 000 → "7.0M rows…" */
  function formatRowCount(count: number): string {
    if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M rows…`;
    if (count >= 1_000) return `${(count / 1_000).toFixed(0)}K rows…`;
    return `${count} rows…`;
  }

  /** Kick off a parse via the worker, showing a decelerating progress loader. */
  function startParse(text: string): void {
    loading = true;
    pendingRows = [];
    startLoaderTicker("Parsing CSV…", "Starting…", {
      ceiling: 85,
      factor: 0.05,
      minStep: 0.2,
      interval: 100,
    });
    serializeWorker.postMessage({ type: "INIT_START" });
    parseWorker.postMessage({ text });
  }

  // Kick off initial parse via worker
  $effect(() => {
    // Serializer worker (initialized first to receive messages)
    serializeWorker = new Worker(
      new URL("../../workers/csvSerializer.worker.ts", import.meta.url),
      { type: "module" },
    );

    serializeWorker.onmessage = (e) => {
      if (e.data.serialized) {
        lastSyncedContent = e.data.serialized;
        content = e.data.serialized;

        // If we were serializing for a view transition, snap to 100 % then hide
        if (editorState.csv.serializing) {
          completeEditorLoader("Done", "", 100, () => {
            editorState.csv.serializing = false;
          });
        }
      } else if (e.data.error) {
        console.error("CSV Serialization Worker Error:", e.data.error);
      }
    };

    // Parser worker
    parseWorker = new Worker(
      new URL("../../workers/csvParser.worker.ts", import.meta.url),
      { type: "module" },
    );

    parseWorker.onmessage = (e) => {
      const msg = e.data;
      switch (msg.type) {
        case "parsed-chunk": {
          const chunk: string[][] = msg.chunk;
          for (let i = 0; i < chunk.length; i++) {
            pendingRows.push(chunk[i]);
          }
          serializeWorker.postMessage({ type: "INIT_CHUNK", chunk });
          // Update sub-message with running row count; progress is
          // driven by the ticker started in startParse().
          editorState.loader.subMessage = formatRowCount(pendingRows.length);
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
          serializeWorker.postMessage({
            type: "INIT_DONE",
            headers: msg.headers,
            delimiter: msg.delimiter,
          });
          pendingRows = [];
          loading = false;
          completeEditorLoader(
            "Building table…",
            `${rowCount.toLocaleString()} rows`,
          );

          // Focus the first cell of the body when parsing completes
          // This typically happens when switching from text mode to table mode.
          if (rowCount > 0 && !csvEditorState.focusedCell) {
            csvEditorState.focusedCell = { rowIndex: 0, colIndex: 0 };
            csvEditorState.navigateAndFocus();
          }
          break;
        }
        case "error":
          stopLoaderTicker();
          console.error("CSV Parse Worker Error:", msg.error);
          loading = false;
          hideEditorLoader();
          break;
      }
    };

    startParse(untrack(() => content));

    return () => {
      stopLoaderTicker();
      debouncedSerialize.cancel();
      parseWorker.terminate();
      serializeWorker.terminate();
      hideEditorLoader();
    };
  });

  // Re-parse when content changes externally (e.g. switching from text mode)
  $effect(() => {
    if (content !== untrack(() => lastSyncedContent)) {
      lastSyncedContent = content;
      history.clear();
      startParse(content);
    }
  });

  // 2. History — structural, lightweight ops
  const history = useCsvHistory();

  // Debounced serialization: sync parsed rows → content string using Web Worker
  const debouncedSerialize = debounce(() => {
    if (!serializeWorker) return;

    serializeWorker.postMessage({
      type: "SERIALIZE",
    });
  }, 500);

  // Trigger debounced serialization whenever history marks dirty.
  $effect(() => {
    if (history.isDirty) {
      history.isDirty = false;
      queueMicrotask(() => debouncedSerialize());
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

  // 5. Editor State (Keyboard, Editing, Focus)
  const csvEditorState = useCsvEditorState(
    history,
    () => parsed,
    (p) => {
      parsed = p;
    },
    () => columns,
    (index) => virtualizer.scrollToIndex(index),
    (ops, reverse) => {
      const workerOps = translateToWorkerOps(ops, reverse);
      serializeWorker?.postMessage({
        type: "UPDATE_OPS",
        ops: workerOps,
      });
    },
  );

  // Deferred transition: when user toggles showTable → false,
  // serialize via worker BEFORE allowing unmount.
  $effect(() => {
    if (!editorState.csv.showTable) {
      if (loading) {
        // If they exit before finishing load, don't serialize, just unmount
        editorState.csv.serializing = false;
        hideEditorLoader();
      } else if (editorState.csv.serializing) {
        startLoaderTicker("Preparing text view…", "", {
          ceiling: 90,
          factor: 0.08,
          minStep: 0.5,
          interval: 80,
        });

        csvEditorState.commitEdit();
        debouncedSerialize.cancel();

        // Send immediate serialization request to the worker
        serializeWorker.postMessage({
          type: "SERIALIZE",
        });
      }
    }
  });

  onDestroy(() => {
    // AGGRESSIVE MEMORY CLEANUP
    parsed = { headers: [], rows: [], delimiter: ",", errors: [] };
    pendingRows = [];
    stableColumns = [];
    tableContainerRef = undefined;
    wrapperRef = undefined;
    if (serializeWorker) {
      serializeWorker.terminate();
    }
    if (parseWorker) {
      parseWorker.terminate();
    }
  });

  // 6. TanStack Table Instance — empty data, only used for column sizing/headers
  let columnSizing = $state<Record<string, number>>({});
  let columnSizingInfo = $state({
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
        return columnSizingInfo as any;
      },
    },
    onColumnSizingChange: (updater) => {
      if (typeof updater === "function") {
        columnSizing = updater(columnSizing);
      } else {
        columnSizing = updater;
      }
    },
    onColumnSizingInfoChange: (updater) => {
      if (typeof updater === "function") {
        columnSizingInfo = updater(columnSizingInfo as any) as any;
      } else {
        columnSizingInfo = updater as any;
      }
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

{#if !loading && !editorState.csv.serializing}
  <CsvContextMenu
    bind:this={contextMenu}
    editorState={csvEditorState}
    container={wrapperRef}
  />
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="csv-table-wrapper"
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
    width: 100%;
    overflow: hidden;
    background: var(--background);
    color: var(--foreground);
    outline: none;
  }

  .csv-table-container {
    flex: 1;
    overflow: auto;
    overscroll-behavior: none;
    position: relative;
  }
</style>
