<script lang="ts">
  import { FlexRender, type Table } from "@tanstack/svelte-table";
  import type { useCsvEditorState } from "./useCsvEditorState.svelte";
  import { hotkey } from "$lib/hotkeys";

  let {
    table,
    editorState,
    totalRows,
    indexColWidth = 50,
  }: {
    table: Table<string[]>;
    editorState: ReturnType<typeof useCsvEditorState>;
    totalRows: number;
    indexColWidth?: number;
  } = $props();
</script>

<!-- Sticky Header -->
<table class="csv-table">
  <colgroup>
    <col style="width: {indexColWidth}px; min-width: {indexColWidth}px;" />
    {#each table.getFlatHeaders() as header}
      <col
        style="width: {header.getSize()}px; min-width: {header.getSize()}px;"
      />
    {/each}
  </colgroup>
  <thead>
    {#if table.getHeaderGroups().length === 0}
      <tr>
        <th
          class="csv-row-num-header"
          data-row="-1"
          data-col="-1"
          style="width: {indexColWidth}px; min-width: {indexColWidth}px; max-width: {indexColWidth}px;"
          >#</th
        >
      </tr>
    {:else}
      {#each table.getHeaderGroups() as headerGroup}
        <tr>
          <th
            class="csv-row-num-header"
            data-row="-1"
            data-col="-1"
            style="width: {indexColWidth}px; min-width: {indexColWidth}px; max-width: {indexColWidth}px;"
            >#</th
          >
          {#each headerGroup.headers as header, colIndex}
            {@const isEditing =
              editorState.editingCell?.rowIndex === -1 &&
              editorState.editingCell?.colIndex === colIndex}
            {@const isFocused =
              editorState.focusedCell?.rowIndex === -1 &&
              editorState.focusedCell?.colIndex === colIndex}
            {@const isSelected =
              editorState.selectionBlock &&
              editorState.selectionBlock.startRow === 0 &&
              editorState.selectionBlock.endRow >= totalRows - 1 &&
              colIndex >= editorState.selectionBlock.startCol &&
              colIndex <= editorState.selectionBlock.endCol}
            <th
              class="csv-cell-header"
              class:focused={isFocused}
              class:selected={isSelected}
              data-row="-1"
              data-col={colIndex}
              style="width: {header.getSize()}px; min-width: {header.getSize()}px; max-width: {header.getSize()}px;"
              ondblclick={() => {
                const title =
                  typeof header.column.columnDef.header === "string"
                    ? header.column.columnDef.header
                    : "";
                editorState.startEditing(-1, colIndex, title);
              }}
              role="columnheader"
              tabindex={isFocused ? 0 : -1}
              aria-selected={isFocused}
            >
              <div class="csv-header-content" class:editing={isEditing}>
                {#if isEditing}
                  <input
                    class="csv-edit-input"
                    type="text"
                    bind:value={editorState.editValue}
                    onblur={() => editorState.commitEdit()}
                    use:hotkey={editorState.editHotkeys}
                  />
                {:else if !header.isPlaceholder}
                  <FlexRender
                    content={header.column.columnDef.header}
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
                ondblclick={(e) => {
                  e.stopPropagation();
                  header.column.resetSize();
                }}
                role="separator"
                aria-orientation="vertical"
                tabindex="-1"
              ></div>
            </th>
          {/each}
        </tr>
      {/each}
    {/if}
  </thead>
</table>

<style>
  .csv-table {
    position: sticky;
    top: 0;
    z-index: 10;
    width: max-content;
    border-collapse: collapse;
    font-size: var(--csv-row-font-size, 13px);
    font-family: "SF Mono", "Fira Code", "JetBrains Mono", Consolas, monospace;
    table-layout: fixed;
  }

  thead th {
    position: relative;
    background: var(--sidebar);
    border-bottom: 2px solid var(--border);
    border-right: 1px solid var(--border);
    padding: 0;
    text-align: left;
    font-weight: 600;
    font-size: var(--csv-header-font-size, 12px);
    height: var(--csv-header-height, 34px);
    color: var(--foreground);
    white-space: nowrap;
    user-select: none;
    outline: none;
    cursor: default;
  }

  thead th.focused {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
    z-index: 10;
    position: relative;
  }

  thead th.selected {
    background-color: color-mix(in srgb, var(--primary) 20%, var(--sidebar));
  }

  thead th:focus-visible {
    outline: none;
  }

  .csv-header-content {
    padding: 6px 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    height: 100%;
    box-sizing: border-box;
    display: flex;
    align-items: center;
  }

  .csv-header-content.editing {
    padding: 0;
  }

  /* Edit input */
  .csv-edit-input {
    width: 100%;
    height: 100%;
    border: none;
    outline: none;
    padding: 5px 10px;
    font-size: inherit;
    font-weight: 600;
    font-family: inherit;
    background: var(--background);
    color: var(--foreground);
    box-sizing: border-box;
  }

  .csv-edit-input:focus,
  .csv-edit-input:focus-visible {
    outline: none;
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
  .csv-row-num-header {
    position: sticky;
    left: 0;
    z-index: 11;
    text-align: right;
    padding: 6px 8px;
    color: var(--muted-foreground);
    font-size: var(--csv-index-font-size, 11px);
    border-right: 1px solid var(--border);
    user-select: none;
    background: var(--sidebar);
  }
</style>
