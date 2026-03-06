<script lang="ts">
  import type { Table } from "@tanstack/svelte-table";
  import type { useScrollVirtualizer } from "./useScrollVirtualizer.svelte";
  import type { useCsvEditorState } from "./useCsvEditorState.svelte";
  import { hotkey } from "$lib/hotkeys";

  type CaretPositionResult = {
    offsetNode: Node;
    offset: number;
  };

  type DocumentWithCaretPositionFromPoint = Document & {
    caretPositionFromPoint?: (
      x: number,
      y: number,
    ) => CaretPositionResult | null;
  };

  let {
    table,
    virtualizer,
    editorState,
    getRow,
    indexColWidth = 50,
  }: {
    table: Table<string[]>;
    virtualizer: ReturnType<typeof useScrollVirtualizer>;
    editorState: ReturnType<typeof useCsvEditorState>;
    getRow: (index: number) => string[] | undefined;
    indexColWidth?: number;
  } = $props();
</script>

<div
  style="position: absolute; top: 0; left: 0; width: 100%; transform: translateY({virtualizer
    .virtualItems[0]?.start ?? 0}px);"
>
  <table class="csv-table csv-table-body">
    <colgroup>
      <col style="width: {indexColWidth}px; min-width: {indexColWidth}px;" />
      {#each table.getFlatHeaders() as header}
        <col
          style="width: {header.getSize()}px; min-width: {header.getSize()}px;"
        />
      {/each}
    </colgroup>
    <tbody>
      {#each virtualizer.virtualItems as virtualRow}
        {@const row = getRow(virtualRow.index)}
        {#if row}
          <tr class:csv-row-even={virtualRow.index % 2 === 0}>
            <td
              class="csv-row-num"
              class:selected={editorState.selectionBlock &&
                virtualRow.index >= editorState.selectionBlock.startRow &&
                virtualRow.index <= editorState.selectionBlock.endRow &&
                editorState.selectionBlock.startCol === 0 &&
                editorState.selectionBlock.endCol >=
                  table.getFlatHeaders().length - 1}
              data-row={virtualRow.index}
              data-col="-1"
              style="height: {virtualRow.size}px; width: {indexColWidth}px; min-width: {indexColWidth}px; max-width: {indexColWidth}px;"
            >
              {virtualRow.index + 1}
            </td>
            {#each table.getFlatHeaders() as header, colIndex}
              {@const isEditing =
                editorState.editingCell?.rowIndex === virtualRow.index &&
                editorState.editingCell?.colIndex === colIndex}
              {@const isFocused =
                editorState.focusedCell?.rowIndex === virtualRow.index &&
                editorState.focusedCell?.colIndex === colIndex}
              {@const isSelected =
                editorState.selectionBlock &&
                virtualRow.index >= editorState.selectionBlock.startRow &&
                virtualRow.index <= editorState.selectionBlock.endRow &&
                colIndex >= editorState.selectionBlock.startCol &&
                colIndex <= editorState.selectionBlock.endCol}
              <td
                style="width: {header.getSize()}px; min-width: {header.getSize()}px; max-width: {header.getSize()}px;"
                class="csv-cell"
                class:focused={isFocused}
                class:selected={isSelected}
                data-row={virtualRow.index}
                data-col={colIndex}
                onclick={(e) => {
                  if (
                    (e.target as HTMLElement).tagName.toLowerCase() === "input"
                  )
                    return;
                  editorState.focusedCell = {
                    rowIndex: virtualRow.index,
                    colIndex,
                  };
                  editorState.navigateAndFocus();
                }}
                ondblclick={(e) => {
                  let offset = (row[colIndex] ?? "").length;
                  const documentWithCaretFallback =
                    document as DocumentWithCaretPositionFromPoint;
                  if (document.caretRangeFromPoint) {
                    const range = document.caretRangeFromPoint(
                      e.clientX,
                      e.clientY,
                    );
                    if (
                      range &&
                      range.startContainer.nodeType === Node.TEXT_NODE
                    ) {
                      offset = range.startOffset;
                    }
                  } else if (documentWithCaretFallback.caretPositionFromPoint) {
                    const pos = documentWithCaretFallback.caretPositionFromPoint(
                      e.clientX,
                      e.clientY,
                    );
                    if (pos && pos.offsetNode.nodeType === Node.TEXT_NODE) {
                      offset = pos.offset;
                    }
                  }
                  editorState.startEditing(
                    virtualRow.index,
                    colIndex,
                    row[colIndex] ?? "",
                    { cursorPosition: offset },
                  );
                }}
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
                    use:hotkey={editorState.editHotkeys}
                  />
                {:else}
                  <div class="csv-cell-content">
                    {row[colIndex] ?? ""}
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
    font-family: "SF Mono", "Fira Code", "JetBrains Mono", Consolas, monospace;
    table-layout: fixed;
  }

  /* Row number column */
  .csv-row-num {
    position: sticky;
    left: 0;
    z-index: 2;
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
    border-bottom: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
  }

  tbody tr:hover {
    background: color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .csv-row-even {
    background-color: var(--muted);
  }

  .csv-cell {
    padding: 0;
    border-right: 1px solid color-mix(in srgb, var(--border) 40%, transparent);
    cursor: default;
    outline: none;
    transition: background-color 0s;
  }

  .csv-cell.selected {
    background-color: color-mix(in srgb, var(--primary) 20%, transparent);
  }

  .csv-row-num.selected {
    background-color: color-mix(in srgb, var(--primary) 20%, transparent);
  }

  .csv-cell:focus-visible {
    outline: none;
  }

  .csv-cell.focused {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
    z-index: 1;
    position: relative;
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
