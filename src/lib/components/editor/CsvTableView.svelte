<script lang="ts">
    import {
        createTable,
        getCoreRowModel,
        type ColumnDef,
        FlexRender,
    } from "@tanstack/svelte-table";
    import { createVirtualizer } from "@tanstack/svelte-virtual";
    import { parseCsv, serializeCsv } from "$lib/utils/editor/csvParser";
    import { tick } from "svelte";

    let { content = $bindable("") } = $props();

    // --- CSV Parsing ---
    let parsed = $derived(parseCsv(content));

    // Track the detected delimiter for serialization
    let detectedDelimiter = $derived(parsed.delimiter);

    // --- TanStack Table Data & Columns ---
    let data = $derived(parsed.rows);
    let columns = $derived(
        parsed.headers.map(
            (header, index): ColumnDef<string[], string> => ({
                id: `col_${index}`,
                accessorFn: (row: string[]) => row[index] ?? "",
                header: header || `Column ${index + 1}`,
                size: 150,
                minSize: 60,
            }),
        ),
    );

    // --- Editing State ---
    let editingCell: { rowIndex: number; colIndex: number } | null =
        $state(null);
    let editValue = $state("");

    function startEditing(rowIndex: number, colIndex: number, value: string) {
        editingCell = { rowIndex, colIndex };
        editValue = value;
        tick().then(() => {
            const input = document.querySelector(
                ".csv-edit-input",
            ) as HTMLInputElement;
            input?.focus();
            input?.select();
        });
    }

    function commitEdit() {
        if (!editingCell) return;
        const { rowIndex, colIndex } = editingCell;

        // Clone rows and update
        const newRows = parsed.rows.map((row) => [...row]);
        if (newRows[rowIndex]) {
            // Ensure the row has enough columns
            while (newRows[rowIndex].length <= colIndex) {
                newRows[rowIndex].push("");
            }
            newRows[rowIndex][colIndex] = editValue;
            // Serialize back to text
            content = serializeCsv(parsed.headers, newRows, detectedDelimiter);
        }
        editingCell = null;
    }

    function cancelEdit() {
        editingCell = null;
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === "Enter") {
            e.preventDefault();
            commitEdit();
        } else if (e.key === "Escape") {
            cancelEdit();
        } else if (e.key === "Tab") {
            e.preventDefault();
            commitEdit();
        }
    }

    // --- TanStack Table ---
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

    // --- Virtualization ---
    let tableContainerRef = $state<HTMLDivElement | undefined>(undefined);

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
        switch (detectedDelimiter) {
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
                return detectedDelimiter;
        }
    });
</script>

<div class="csv-table-wrapper">
    <!-- Info Bar -->
    <div class="csv-info-bar">
        <span class="csv-info-stat">
            {parsed.rows.length} rows × {parsed.headers.length} cols
        </span>
        <span class="csv-info-delimiter">
            Delimiter: <strong>{delimiterLabel}</strong>
        </span>
        {#if parsed.errors.length > 0}
            <span class="csv-info-errors">
                ⚠ {parsed.errors.length} parse error{parsed.errors.length > 1
                    ? "s"
                    : ""}
            </span>
        {/if}
    </div>

    <!-- Table Container -->
    <div class="csv-table-container" bind:this={tableContainerRef}>
        <div
            style="height: {$virtualizer.getTotalSize()}px; width: 100%; position: relative;"
        >
            <!-- Sticky Header -->
            <table class="csv-table">
                <thead>
                    {#each table.getHeaderGroups() as headerGroup}
                        <tr>
                            <th class="csv-row-num-header">#</th>
                            {#each headerGroup.headers as header}
                                <th
                                    style="width: {header.getSize()}px; min-width: {header.getSize()}px;"
                                >
                                    <div class="csv-header-content">
                                        {#if !header.isPlaceholder}
                                            <FlexRender
                                                content={header.column.columnDef
                                                    .header}
                                                context={header.getContext()}
                                            />
                                        {/if}
                                    </div>
                                    <!-- Resize handle -->
                                    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                                    <div
                                        class="csv-resize-handle"
                                        class:resizing={header.column.getIsResizing()}
                                        onmousedown={header.getResizeHandler()}
                                        ontouchstart={header.getResizeHandler()}
                                        role="separator"
                                        aria-orientation="vertical"
                                        tabindex="-1"
                                    ></div>
                                </th>
                            {/each}
                        </tr>
                    {/each}
                </thead>
            </table>

            <!-- Virtualized Body -->
            <div
                style="position: absolute; top: 0; left: 0; width: 100%; transform: translateY({$virtualizer.getVirtualItems()[0]
                    ?.start ?? 0}px); padding-top: 33px;"
            >
                <table class="csv-table csv-table-body">
                    <tbody>
                        {#each $virtualizer.getVirtualItems() as virtualRow}
                            {@const row =
                                table.getRowModel().rows[virtualRow.index]}
                            {#if row}
                                <tr
                                    class:csv-row-even={virtualRow.index % 2 ===
                                        0}
                                >
                                    <td class="csv-row-num">
                                        {virtualRow.index + 1}
                                    </td>
                                    {#each row.getVisibleCells() as cell, colIndex}
                                        {@const isEditing =
                                            editingCell?.rowIndex ===
                                                virtualRow.index &&
                                            editingCell?.colIndex === colIndex}
                                        <td
                                            style="width: {cell.column.getSize()}px; min-width: {cell.column.getSize()}px;"
                                            class="csv-cell"
                                            ondblclick={() =>
                                                startEditing(
                                                    virtualRow.index,
                                                    colIndex,
                                                    cell.getValue() as string,
                                                )}
                                            role="gridcell"
                                            tabindex="-1"
                                        >
                                            {#if isEditing}
                                                <input
                                                    class="csv-edit-input"
                                                    type="text"
                                                    bind:value={editValue}
                                                    onblur={() => commitEdit()}
                                                    onkeydown={handleKeydown}
                                                />
                                            {:else}
                                                <div class="csv-cell-content">
                                                    {cell.getValue()}
                                                </div>
                                            {/if}
                                        </td>
                                    {/each}
                                </tr>
                            {/if}
                        {/each}
                    </tbody>
                </table>
            </div>
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
    }

    .csv-info-bar {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 4px 12px;
        font-size: 11px;
        font-weight: 500;
        color: var(--muted-foreground);
        border-bottom: 1px solid var(--border);
        background: var(--sidebar);
        flex-shrink: 0;
        user-select: none;
    }

    .csv-info-errors {
        color: hsl(0, 80%, 60%);
    }

    .csv-table-container {
        flex: 1;
        overflow: auto;
        overscroll-behavior: none;
        position: relative;
    }

    .csv-table {
        width: max-content;
        min-width: 100%;
        border-collapse: collapse;
        font-size: 13px;
        font-family: "SF Mono", "Fira Code", "JetBrains Mono", Consolas,
            monospace;
        table-layout: fixed;
    }

    /* Sticky header */
    thead {
        position: sticky;
        top: 0;
        z-index: 2;
    }

    thead th {
        position: relative;
        background: var(--sidebar);
        border-bottom: 2px solid var(--border);
        border-right: 1px solid var(--border);
        padding: 6px 10px;
        text-align: left;
        font-weight: 600;
        font-size: 12px;
        color: var(--foreground);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        user-select: none;
    }

    .csv-header-content {
        overflow: hidden;
        text-overflow: ellipsis;
    }

    /* Resize handle */
    .csv-resize-handle {
        position: absolute;
        right: 0;
        top: 0;
        height: 100%;
        width: 4px;
        cursor: col-resize;
        user-select: none;
        touch-action: none;
        opacity: 0;
        transition: opacity 0.15s ease;
    }

    .csv-resize-handle:hover,
    .csv-resize-handle.resizing {
        opacity: 1;
        background: var(--primary);
    }

    /* Row number column */
    .csv-row-num-header,
    .csv-row-num {
        width: 50px;
        min-width: 50px;
        max-width: 50px;
        text-align: right;
        padding: 6px 8px;
        color: var(--muted-foreground);
        font-size: 11px;
        border-right: 1px solid var(--border);
        user-select: none;
    }

    .csv-row-num-header {
        background: var(--sidebar);
    }

    .csv-row-num {
        background: var(--sidebar);
    }

    /* Body rows */
    tbody tr {
        border-bottom: 1px solid
            color-mix(in srgb, var(--border) 50%, transparent);
    }

    tbody tr:hover {
        background: color-mix(in srgb, var(--accent) 40%, transparent);
    }

    .csv-row-even {
        background: color-mix(in srgb, var(--muted) 30%, transparent);
    }

    /* Cells */
    .csv-cell {
        padding: 0;
        border-right: 1px solid
            color-mix(in srgb, var(--border) 40%, transparent);
        cursor: default;
    }

    .csv-cell-content {
        padding: 5px 10px;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    /* Edit input */
    .csv-edit-input {
        width: 100%;
        border: none;
        outline: 2px solid var(--primary);
        outline-offset: -2px;
        padding: 5px 10px;
        font-size: 13px;
        font-family: inherit;
        background: var(--background);
        color: var(--foreground);
        box-sizing: border-box;
    }
</style>
