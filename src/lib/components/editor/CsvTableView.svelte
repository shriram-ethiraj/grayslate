<script lang="ts">
    import {
        createTable,
        getCoreRowModel,
        type ColumnDef,
    } from "@tanstack/svelte-table";
    import { createVirtualizer } from "@tanstack/svelte-virtual";
    import { parseCsv, serializeCsv } from "$lib/utils/editor/csvParser";
    import { debounce } from "lodash-es";

    // Extracted logic and components
    import { useCsvHistory } from "./csv/useCsvHistory.svelte";
    import { useCsvEditorState } from "./csv/useCsvEditorState.svelte";
    import CsvTableHeader from "./csv/CsvTableHeader.svelte";
    import CsvTableBody from "./csv/CsvTableBody.svelte";

    let {
        content = $bindable(""),
        tableInfo = $bindable({ rows: 0, cols: 0, delimiter: "", errors: 0 }),
    } = $props();

    // 1. Parse once on mount — parsed data is the mutable source of truth
    let parsed = $state(parseCsv(content));

    // Track the last content we synced from, to detect external changes
    let lastSyncedContent = $state(content);
    let worker: Worker;

    $effect(() => {
        // Initialize Web Worker for background serialization
        worker = new Worker(
            new URL("../../workers/csvSerializer.worker.ts", import.meta.url),
            { type: "module" },
        );

        worker.onmessage = (e) => {
            if (e.data.serialized) {
                lastSyncedContent = e.data.serialized;
                content = e.data.serialized;
            } else if (e.data.error) {
                console.error("CSV Serialization Worker Error:", e.data.error);
            }
        };

        return () => {
            // On unmount: commit any in-progress edit, cancel pending debounce,
            // and synchronously serialize so content is up-to-date for the editor.
            editorState.commitEdit();
            debouncedSerialize.cancel();
            const snapshot = fastCloneParsed();
            content = serializeCsv(
                snapshot.headers,
                snapshot.rows,
                snapshot.delimiter,
            );
            lastSyncedContent = content;
            worker.terminate();
        };
    });

    // Re-parse when content changes externally (e.g. switching from text mode)
    $effect(() => {
        if (content !== lastSyncedContent) {
            parsed = parseCsv(content);
            lastSyncedContent = content;
            history.clear();
        }
    });

    // 2. History — structural, lightweight ops
    const history = useCsvHistory();

    // Svelte 5's generic $state.snapshot() is extremely slow for large 2D arrays
    // because it recursively unwraps proxies and checks object types.
    // This manual clone is heavily optimized for a strict `string[][]` structure.
    function fastCloneParsed() {
        const sourceRows = parsed.rows;
        const length = sourceRows.length;
        const rows = new Array(length);

        for (let i = 0; i < length; i++) {
            const row = sourceRows[i];
            const rowLen = row.length;
            const newRow = new Array(rowLen);
            for (let j = 0; j < rowLen; j++) {
                newRow[j] = row[j];
            }
            rows[i] = newRow;
        }

        return {
            headers: parsed.headers.slice(),
            rows,
            delimiter: parsed.delimiter,
        };
    }

    // Debounced serialization: sync parsed rows → content string using Web Worker
    const debouncedSerialize = debounce(() => {
        if (!worker) return;
        // Use manual clone to bypass Svelte 5's slow deep clone and avoid DataCloneError
        const snapshot = fastCloneParsed();
        worker.postMessage({
            headers: snapshot.headers,
            rows: snapshot.rows,
            delimiter: snapshot.delimiter,
        });
    }, 500);

    // Trigger debounced serialization whenever history marks dirty.
    // queueMicrotask defers the call so the current reactive flush
    // (which needs to complete for focus to move) isn't blocked.
    $effect(() => {
        if (history.isDirty) {
            history.isDirty = false;
            queueMicrotask(() => debouncedSerialize());
        }
    });

    // 3. TanStack Table Data & Columns
    let data = $derived(parsed.rows);

    // Stabilise columns reference — only rebuild when headers actually change
    // by value, not just by object reference. Cell edits replace `parsed` but
    // leave `headers` identical, so this prevents a full TanStack row-model
    // rebuild on every keystroke.
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
    const editorState = useCsvEditorState(
        history,
        () => parsed,
        (p) => {
            parsed = p;
        },
        () => columns,
        (index, options) => $virtualizer.scrollToIndex(index, options),
    );

    // 6. TanStack Table Instance
    const table = createTable({
        get data() {
            return data;
        },
        get columns() {
            return columns;
        },
        getCoreRowModel: getCoreRowModel(),
        columnResizeMode: "onChange" as const,
    });

    // 7. Virtualizer Instance
    const virtualizer = createVirtualizer({
        count: table.getRowModel().rows.length,
        getScrollElement: () => tableContainerRef ?? null,
        estimateSize: () => 32,
        overscan: 20,
    });

    $effect(() => {
        $virtualizer.setOptions({
            count: table.getRowModel().rows.length,
            getScrollElement: () => tableContainerRef ?? null,
            estimateSize: () => 32,
            overscan: 20,
        });
    });

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

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
    class="csv-table-wrapper"
    tabindex="0"
    role="grid"
    onkeydown={(e) => editorState.handleCellKeydown(e)}
    onclick={(e) => {
        // If clicking outside a cell, give focus to wrapper for keyboard capture
        const target = e.target as HTMLElement;
        if (!target.closest(".csv-cell")) {
            // Keep existing focusedCell or default to 0,0
            if (!editorState.focusedCell && parsed.rows.length > 0) {
                editorState.focusedCell = { rowIndex: 0, colIndex: 0 };
                editorState.navigateAndFocus();
            }
        }
    }}
>
    <!-- Table Container -->
    <div class="csv-table-container" bind:this={tableContainerRef}>
        <div
            style="height: {$virtualizer.getTotalSize()}px; width: 100%; position: relative;"
        >
            <CsvTableHeader {table} {editorState} />
            <CsvTableBody {table} virtualizer={$virtualizer} {editorState} />
        </div>
    </div>
</div>

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
