<script lang="ts">
  import { tick } from "svelte";
  import CopyIcon from "~icons/lucide/copy";
  import TextSelect from "~icons/lucide/text-select";
  import { registerHotkey } from "$lib/hotkeys";
  import { formatForDisplay } from "@tanstack/hotkeys";
  import {
    activateMarkdownPreview,
    copyMarkdownPreviewSelection,
    focusMarkdownPreview,
    hasMarkdownPreviewSelection,
    selectAllMarkdownPreview,
  } from "./previewActions";

  let { previewEl }: { previewEl: HTMLElement | undefined } = $props();

  let open = $state(false);
  let menuX = $state(0);
  let menuY = $state(0);
  let menuRef = $state<HTMLDivElement | null>(null);
  let hasSelection = $state(false);

  const itemBase =
    "relative flex w-full items-center rounded-sm px-2 py-1.5 text-sm outline-hidden select-none";
  const itemEnabled = `${itemBase} cursor-pointer hover:bg-accent hover:text-accent-foreground`;

  function openMenu(event: MouseEvent) {
    activateMarkdownPreview();
    hasSelection = hasMarkdownPreviewSelection();
    open = true;
    menuX = event.clientX;
    menuY = event.clientY;

    tick().then(() => {
      if (!menuRef) return;

      const rect = menuRef.getBoundingClientRect();
      const viewportWidth = window.innerWidth;
      const viewportHeight = window.innerHeight;

      if (menuX + rect.width > viewportWidth) {
        menuX = viewportWidth - rect.width - 4;
      }
      if (menuY + rect.height > viewportHeight) {
        menuY = viewportHeight - rect.height - 4;
      }
    });
  }

  function close() {
    if (!open) return;
    open = false;
    focusMarkdownPreview();
  }

  async function handleCopy() {
    const copied = await copyMarkdownPreviewSelection();
    if (!copied) return;
    close();
  }

  function handleSelectAll() {
    selectAllMarkdownPreview();
    close();
  }

  $effect(() => {
    if (!previewEl) return;

    function onContextMenu(event: MouseEvent) {
      event.preventDefault();
      activateMarkdownPreview();
      focusMarkdownPreview();
      openMenu(event);
    }

    previewEl.addEventListener("contextmenu", onContextMenu);
    return () => previewEl.removeEventListener("contextmenu", onContextMenu);
  });

  $effect(() => {
    if (!open) return;

    function handlePointerDismiss(event: PointerEvent) {
      if (menuRef && menuRef.contains(event.target as Node)) return;
      if (event.button === 2 && previewEl?.contains(event.target as Node)) return;
      close();
    }

    window.addEventListener("pointerdown", handlePointerDismiss);
    const cleanupEscape = registerHotkey(
      "Escape",
      (event) => {
        event.preventDefault();
        close();
      },
      { ignoreInputs: false },
    );

    return () => {
      window.removeEventListener("pointerdown", handlePointerDismiss);
      cleanupEscape();
    };
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_no_static_element_interactions a11y_interactive_supports_focus -->
  <div
    bind:this={menuRef}
    class="fixed z-50 min-w-44 flex flex-col gap-0.5 rounded-md border bg-popover p-1 text-popover-foreground shadow-md animate-in fade-in-0 zoom-in-95"
    style="left: {menuX}px; top: {menuY}px;"
    role="menu"
    tabindex="-1"
    oncontextmenu={(event) => event.preventDefault()}
  >
    {#if hasSelection}
      <button class={itemEnabled} role="menuitem" onclick={handleCopy}>
        <CopyIcon class="mr-2 h-4 w-4 shrink-0" />
        <span>Copy</span>
        <span class="ml-auto pl-4 text-xs text-muted-foreground"
          >{formatForDisplay("Mod+C")}</span
        >
      </button>

      <div class="my-1 h-px bg-muted"></div>
    {/if}

    <button class={itemEnabled} role="menuitem" onclick={handleSelectAll}>
      <TextSelect class="mr-2 h-4 w-4 shrink-0" />
      <span>Select All</span>
      <span class="ml-auto pl-4 text-xs text-muted-foreground"
        >{formatForDisplay("Mod+A")}</span
      >
    </button>
  </div>
{/if}