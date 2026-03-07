<script lang="ts">
  import { tick } from "svelte";
  import Plus from "~icons/lucide/plus";
  import ArrowLeft from "~icons/lucide/arrow-left";
  import ArrowRight from "~icons/lucide/arrow-right";
  import ArrowUp from "~icons/lucide/arrow-up";
  import ArrowDown from "~icons/lucide/arrow-down";
  import Trash2 from "~icons/lucide/trash-2";
  import type { useCsvEditorState } from "./useCsvEditorState.svelte";
  import { registerHotkey } from "$lib/hotkeys";
  import { formatForDisplay } from "@tanstack/hotkeys";

  let {
    editorState,
    container,
  }: {
    editorState: ReturnType<typeof useCsvEditorState>;
    container: HTMLElement | undefined;
  } = $props();

  let open = $state(false);
  let menuX = $state(0);
  let menuY = $state(0);
  let menuRef = $state<HTMLDivElement | null>(null);
  let menuMode = $state<"selection" | "insert-column">("selection");

  const itemBase =
    "relative flex w-full items-center rounded-sm px-2 py-1.5 text-sm outline-hidden select-none";
  const itemEnabled = `${itemBase} cursor-pointer hover:bg-accent hover:text-accent-foreground`;
  const destructiveItem = `${itemEnabled} text-destructive`;

  const isRowSelection = $derived(editorState.isRowSelection());
  const isColumnSelection = $derived(editorState.isColumnSelection());
  const canMoveRowUp = $derived.by(() => {
    editorState.selectionBlock;
    editorState.focusedCell;
    return editorState.canMoveSelectedRowsUp();
  });
  const canMoveRowDown = $derived.by(() => {
    editorState.selectionBlock;
    editorState.focusedCell;
    return editorState.canMoveSelectedRowsDown();
  });
  const canMoveColumnLeft = $derived.by(() => {
    editorState.selectionBlock;
    editorState.focusedCell;
    return editorState.canMoveSelectedColumnsLeft();
  });
  const canMoveColumnRight = $derived.by(() => {
    editorState.selectionBlock;
    editorState.focusedCell;
    return editorState.canMoveSelectedColumnsRight();
  });

  export function openMenu(
    x: number,
    y: number,
    options?: { mode?: "selection" | "insert-column" },
  ) {
    const nextMode = options?.mode ?? "selection";
    if (nextMode === "selection" && !editorState.selectionBlock) return;

    menuMode = nextMode;
    open = true;
    menuX = x;
    menuY = y;

    tick().then(() => {
      if (!menuRef) return;
      const rect = menuRef.getBoundingClientRect();
      const vw = window.innerWidth;
      const vh = window.innerHeight;
      if (menuX + rect.width > vw) menuX = vw - rect.width - 4;
      if (menuY + rect.height > vh) menuY = vh - rect.height - 4;
    });
  }

  function close() {
    if (!open) return;
    open = false;
    container?.focus();
  }

  $effect(() => {
    if (!open) return;
    function handlePointerDismiss(e: PointerEvent) {
      if (menuRef && menuRef.contains(e.target as Node)) return;
      // If right click, let parent handle reopening
      if (e.button === 2 && container?.contains(e.target as Node)) return;
      close();
    }
    window.addEventListener("pointerdown", handlePointerDismiss);
    const cleanupEscape = registerHotkey(
      "Escape",
      (e) => {
        e.preventDefault();
        close();
      },
      { ignoreInputs: false },
    );
    return () => {
      window.removeEventListener("pointerdown", handlePointerDismiss);
      cleanupEscape();
    };
  });

  function handleDelete() {
    close();
    if (isRowSelection) {
      editorState.deleteSelectedRows();
      return;
    }
    if (isColumnSelection) {
      editorState.deleteSelectedColumns();
      return;
    }
    editorState.deleteSelection();
  }

  function handleInsertColumnLeft() {
    close();
    editorState.addColumnLeft();
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_no_static_element_interactions a11y_interactive_supports_focus -->
  <div
    bind:this={menuRef}
    class="fixed z-50 min-w-44 flex flex-col gap-0.5 rounded-md border bg-popover p-1 text-popover-foreground shadow-md animate-in fade-in-0 zoom-in-95"
    style="left: {menuX}px; top: {menuY}px;"
    role="menu"
    tabindex="-1"
    oncontextmenu={(e) => e.preventDefault()}
  >
    {#if menuMode === "insert-column"}
      <button class={itemEnabled} role="menuitem" onclick={handleInsertColumnLeft}>
        <Plus class="mr-2 h-4 w-4 shrink-0" />
        <span>Insert Column Left</span>
      </button>
    {:else if isRowSelection}
      <button class={itemEnabled} role="menuitem" onclick={handleInsertColumnLeft}>
        <Plus class="mr-2 h-4 w-4 shrink-0" />
        <span>Insert Column Left</span>
      </button>
      <button class={itemEnabled} role="menuitem" onclick={() => {
        close();
        editorState.addRowAbove();
      }}>
        <Plus class="mr-2 h-4 w-4 shrink-0" />
        <span>Insert Row Above</span>
      </button>
      <button class={itemEnabled} role="menuitem" onclick={() => {
        close();
        editorState.addRowBelow();
      }}>
        <Plus class="mr-2 h-4 w-4 shrink-0" />
        <span>Insert Row Below</span>
      </button>
      {#if canMoveRowUp}
        <button class={itemEnabled} role="menuitem" onclick={() => {
          close();
          editorState.moveSelectedRowsUp();
        }}>
          <ArrowUp class="mr-2 h-4 w-4 shrink-0" />
          <span>Move Row Up</span>
          <span class="ml-auto pl-4 text-xs text-muted-foreground"
            >{formatForDisplay("Alt+ArrowUp")}</span
          >
        </button>
      {/if}
      {#if canMoveRowDown}
        <button class={itemEnabled} role="menuitem" onclick={() => {
          close();
          editorState.moveSelectedRowsDown();
        }}>
          <ArrowDown class="mr-2 h-4 w-4 shrink-0" />
          <span>Move Row Down</span>
          <span class="ml-auto pl-4 text-xs text-muted-foreground"
            >{formatForDisplay("Alt+ArrowDown")}</span
          >
        </button>
      {/if}
      <button class={destructiveItem} role="menuitem" onclick={handleDelete}>
        <Trash2 class="mr-2 h-4 w-4 shrink-0" />
        <span>Delete Row</span>
        <span class="ml-auto pl-4 text-xs text-muted-foreground"
          >{formatForDisplay("Delete")}</span
        >
      </button>
    {:else if isColumnSelection}
      <button class={itemEnabled} role="menuitem" onclick={handleInsertColumnLeft}>
        <Plus class="mr-2 h-4 w-4 shrink-0" />
        <span>Insert Column Left</span>
      </button>
      <button class={itemEnabled} role="menuitem" onclick={() => {
        close();
        editorState.addColumnRight();
      }}>
        <Plus class="mr-2 h-4 w-4 shrink-0" />
        <span>Insert Column Right</span>
      </button>
      {#if canMoveColumnLeft}
        <button class={itemEnabled} role="menuitem" onclick={() => {
          close();
          editorState.moveSelectedColumnsLeft();
        }}>
          <ArrowLeft class="mr-2 h-4 w-4 shrink-0" />
          <span>Move Column Left</span>
          <span class="ml-auto pl-4 text-xs text-muted-foreground"
            >{formatForDisplay("Alt+ArrowLeft")}</span
          >
        </button>
      {/if}
      {#if canMoveColumnRight}
        <button class={itemEnabled} role="menuitem" onclick={() => {
          close();
          editorState.moveSelectedColumnsRight();
        }}>
          <ArrowRight class="mr-2 h-4 w-4 shrink-0" />
          <span>Move Column Right</span>
          <span class="ml-auto pl-4 text-xs text-muted-foreground"
            >{formatForDisplay("Alt+ArrowRight")}</span
          >
        </button>
      {/if}
      <button class={destructiveItem} role="menuitem" onclick={handleDelete}>
        <Trash2 class="mr-2 h-4 w-4 shrink-0" />
        <span>Delete Column</span>
        <span class="ml-auto pl-4 text-xs text-muted-foreground"
          >{formatForDisplay("Delete")}</span
        >
      </button>
    {/if}
  </div>
{/if}
