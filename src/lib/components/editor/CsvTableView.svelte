<script lang="ts">
    import {
        createTable,
        getCoreRowModel,
        type ColumnDef,
    } from "@tanstack/svelte-table";
    import { useScrollVirtualizer } from "./csv/useScrollVirtualizer.svelte";
    import { editorState, showEditorLoader, hideEditorLoader } from "$lib/state/editor.svelte";
    import { debounce } from "lodash-es";
    import { untrack } from "svelte";
    import { useCsvHistory } from "./csv/useCsvHistory.svelte";
    import { useCsvEditorState } from "./csv/useCsvEditorState.svelte";
    import CsvTableHeader from "./csv/CsvTableHeader.svelte";
    import CsvTableBody from "./csv/CsvTableBody.svelte";

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

                // If we were serializing for a view transition, complete it
                if (editorState.csv.serializing) {
                    editorState.csv.serializing = false;
                    hideEditorLoader();
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
                    showEditorLoader(
                        "Parsing CSV…",
                        `${(pendingRows.length / 1_000_000).toFixed(1)}M rows loaded…`,
                    );
                    break;
                }
                case "parsed-complete":
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
                    hideEditorLoader();
                    break;
                case "error":
                    console.error("CSV Parse Worker Error:", msg.error);
                    loading = false;
                    hideEditorLoader();
                    break;
            }
        };

        // Start initial parse
        loading = true;
        showEditorLoader("Parsing CSV…", "Starting…");
        serializeWorker.postMessage({ type: "INIT_START" });
        parseWorker.postMessage({ text: untrack(() => content) });

        return () => {
            // On unmount: just terminate workers.
            // Serialization is handled by the showTable watcher above.
            debouncedSerialize.cancel();
            parseWorker.terminate();
            serializeWorker.terminate();
            hideEditorLoader();
        };
    });

    // Re-parse when content changes externally (e.g. switching from text mode)
    $effect(() => {
        if (content !== untrack(() => lastSyncedContent)) {
            loading = true;
            showEditorLoader("Parsing CSV…", "Starting…");
            lastSyncedContent = content;
            history.clear();
            serializeWorker?.postMessage({ type: "INIT_START" });
            parseWorker?.postMessage({ text: content });
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
            serializeWorker?.postMessage({
                type: "UPDATE_OPS",
                ops,
                reverse,
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
                // Trigger the serialization if not loading and serializing is true
                showEditorLoader("Preparing text view…");
                csvEditorState.commitEdit();
                debouncedSerialize.cancel();

                // Send immediate serialization request to the worker
                serializeWorker.postMessage({
                    type: "SERIALIZE",
                });
            }
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
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
        class="csv-table-wrapper"
        tabindex="0"
        role="grid"
        onkeydown={(e) => csvEditorState.handleCellKeydown(e)}
        onclick={(e) => {
            const target = e.target as HTMLElement;
            if (!target.closest(".csv-cell")) {
                if (!csvEditorState.focusedCell && parsed.rows.length > 0) {
                    csvEditorState.focusedCell = { rowIndex: 0, colIndex: 0 };
                    csvEditorState.navigateAndFocus();
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
        height: 100%;
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
