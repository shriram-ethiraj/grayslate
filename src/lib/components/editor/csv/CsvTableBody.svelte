<script lang="ts">
    import type { Table } from "@tanstack/svelte-table";
    import { type SvelteVirtualizer } from "@tanstack/svelte-virtual";
    import type { useCsvEditorState } from "./useCsvEditorState.svelte";

    let {
        table,
        virtualizer,
        editorState,
    }: {
        table: Table<string[]>;
        virtualizer: SvelteVirtualizer<HTMLDivElement, Element>;
        editorState: ReturnType<typeof useCsvEditorState>;
    } = $props();

    let rows = $derived(table.getRowModel().rows);
</script>

<div
    style="position: absolute; top: 0; left: 0; width: 100%; transform: translateY({virtualizer.getVirtualItems()[0]
        ?.start ?? 0}px); padding-top: 33px;"
>
    <table class="csv-table csv-table-body">
        <colgroup>
            <col style="width: 50px; min-width: 50px;" />
            {#each table.getFlatHeaders() as header}
                <col
                    style="width: {header.getSize()}px; min-width: {header.getSize()}px;"
                />
            {/each}
        </colgroup>
        <tbody>
            {#each virtualizer.getVirtualItems() as virtualRow}
                {@const row = rows[virtualRow.index]}
                {#if row}
                    <tr class:csv-row-even={virtualRow.index % 2 === 0}>
                        <td class="csv-row-num">
                            {virtualRow.index + 1}
                        </td>
                        {#each row.getVisibleCells() as cell, colIndex}
                            {@const isEditing =
                                editorState.editingCell?.rowIndex ===
                                    virtualRow.index &&
                                editorState.editingCell?.colIndex === colIndex}
                            {@const isFocused =
                                editorState.focusedCell?.rowIndex ===
                                    virtualRow.index &&
                                editorState.focusedCell?.colIndex === colIndex}
                            <td
                                style="width: {cell.column.getSize()}px; min-width: {cell.column.getSize()}px; max-width: {cell.column.getSize()}px;"
                                class="csv-cell"
                                class:focused={isFocused}
                                data-row={virtualRow.index}
                                data-col={colIndex}
                                onclick={() => {
                                    editorState.focusedCell = {
                                        rowIndex: virtualRow.index,
                                        colIndex,
                                    };
                                }}
                                ondblclick={() =>
                                    editorState.startEditing(
                                        virtualRow.index,
                                        colIndex,
                                        row.original[colIndex],
                                    )}
                                role="gridcell"
                                tabindex={isFocused ? 0 : -1}
                                aria-selected={isFocused}
                            >
                                {#if isEditing}
                                    <input
                                        class="csv-edit-input"
                                        type="text"
                                        bind:value={editorState.editValue}
                                        onblur={() => editorState.commitEdit()}
                                        onkeydown={(e) =>
                                            editorState.handleEditKeydown(e)}
                                    />
                                {:else}
                                    <div class="csv-cell-content">
                                        {row.original[colIndex]}
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

<style>
    .csv-table {
        width: max-content;
        border-collapse: collapse;
        font-size: 13px;
        font-family: "SF Mono", "Fira Code", "JetBrains Mono", Consolas,
            monospace;
        table-layout: fixed;
    }

    /* Row number column */
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
        background-color: var(--muted);
    }

    /* Cells */
    .csv-cell {
        padding: 0;
        border-right: 1px solid
            color-mix(in srgb, var(--border) 40%, transparent);
        cursor: default;
        outline: none;
    }

    .csv-cell.focused {
        outline: 2px solid var(--primary);
        outline-offset: -2px;
        z-index: 1;
        position: relative;
    }

    .csv-cell:focus-visible {
        outline: none;
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
        outline: none;
        padding: 5px 10px;
        font-size: 13px;
        font-family: inherit;
        background: var(--background);
        color: var(--foreground);
        box-sizing: border-box;
    }

    .csv-edit-input:focus,
    .csv-edit-input:focus-visible {
        outline: none;
    }
</style>
