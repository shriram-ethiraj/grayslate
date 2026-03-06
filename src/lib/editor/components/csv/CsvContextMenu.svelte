<script lang="ts">
  import { tick } from "svelte";
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

  const itemBase =
    "relative flex w-full items-center rounded-sm px-2 py-1.5 text-sm outline-hidden select-none";
  const itemEnabled = `${itemBase} cursor-pointer hover:bg-accent hover:text-accent-foreground`;

  export function openMenu(x: number, y: number) {
    if (!editorState.selectionBlock) return;
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
    editorState.deleteSelection();
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
    <button class={itemEnabled} role="menuitem" onclick={handleDelete}>
      <Trash2 class="mr-2 h-4 w-4 shrink-0 text-destructive" />
      <span class="text-destructive">Delete</span>
      <span class="ml-auto pl-4 text-xs text-muted-foreground"
        >{formatForDisplay("Delete")}</span
      >
    </button>
  </div>
{/if}
