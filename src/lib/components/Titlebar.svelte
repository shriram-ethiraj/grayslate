<script lang="ts">
  import { Window } from "@tauri-apps/api/window";
  import { type } from "@tauri-apps/plugin-os";
  import { emit, listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import * as Menubar from "$lib/components/ui/menubar/index.js";
  import Check from "~icons/lucide/check";
  import Square from "~icons/lucide/square";
  import Minimize2 from "~icons/lucide/minimize-2";
  import Minus from "~icons/lucide/minus";
  import X from "~icons/lucide/x";

  import { editorState } from "$lib/state/editor.svelte";
  import {
    editorUndo,
    editorRedo,
    editorCut,
    editorCopy,
    editorPaste,
    editorSelectAll,
  } from "$lib/editor/core/actions";

  let osType = $state("");
  const appWindow = new Window("main");

  let isMaximized = $state(false);
  let unlistenResize: (() => void) | undefined;

  const isMac = $derived(osType === "macos");
  /** Platform modifier key label */
  const mod = $derived(isMac ? "⌘" : "Ctrl");
  /** Redo shortcut differs between platforms */
  const redoShortcut = $derived(isMac ? `${mod}+Shift+Z` : `${mod}+Y`);
  const wordWrapShortcut = $derived(isMac ? "⌥+Z" : "Alt+Z");

  // Unlisten callback for the macOS native menu edit-action event.
  let unlistenEditAction: (() => void) | undefined;
  let unlistenWordWrap: (() => void) | undefined;

  onMount(async () => {
    osType = await type();

    // Track maximize state for Windows/Linux controls
    isMaximized = await appWindow.isMaximized();
    unlistenResize = await appWindow.onResized(async () => {
      isMaximized = await appWindow.isMaximized();
    });

    // On macOS, the in-window Menubar is hidden and the system menu bar is
    // used instead. Forward native menu edit events to the same handlers
    // used by the custom Menubar on Windows/Linux.
    if (osType === "macos") {
      unlistenEditAction = await listen<string>(
        "menu://edit-action",
        (event) => {
          handleEdit(event.payload);
        },
      );

      // Word wrap uses a separate event that carries the actual
      // checked boolean from the native CheckMenuItem.  Setting
      // the state directly (instead of toggling) keeps the two
      // sides in lock-step regardless of muda's auto-toggle.
      unlistenWordWrap = await listen<boolean>(
        "menu://word-wrap-state",
        (event) => {
          editorState.wordWrap = event.payload;
        },
      );
    }
  });

  onDestroy(() => {
    unlistenEditAction?.();
    unlistenWordWrap?.();
    unlistenResize?.();
  });

  async function handleOpen() {
    await emit("menu://open-file");
  }

  async function handleEdit(action: string) {
    const view = editorState.activeView;
    if (!view) return;

    switch (action) {
      case "undo":
        editorUndo(view);
        break;
      case "redo":
        editorRedo(view);
        break;
      case "cut":
        await editorCut(view);
        break;
      case "copy":
        await editorCopy(view);
        break;
      case "paste":
        await editorPaste(view);
        break;
      case "selectAll":
        editorSelectAll(view);
        break;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.altKey && e.key.toLowerCase() === "z") {
      if (isMac) return;
      e.preventDefault();
      editorState.wordWrap = !editorState.wordWrap;
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#snippet appMenubar()}
  <Menubar.Root class="pointer-events-auto border-none bg-transparent">
    <Menubar.Menu>
      <Menubar.Trigger class="cursor-pointer">File</Menubar.Trigger>
      <Menubar.Content>
        <Menubar.Item onclick={handleOpen}>
          Open File...
          <Menubar.Shortcut>{mod}+O</Menubar.Shortcut>
        </Menubar.Item>
      </Menubar.Content>
    </Menubar.Menu>
    <Menubar.Menu>
      <Menubar.Trigger class="cursor-pointer">Edit</Menubar.Trigger>
      <Menubar.Content>
        <Menubar.Item onclick={() => handleEdit("undo")}
          >Undo<Menubar.Shortcut>{mod}+Z</Menubar.Shortcut></Menubar.Item
        >
        <Menubar.Item onclick={() => handleEdit("redo")}
          >Redo<Menubar.Shortcut>{redoShortcut}</Menubar.Shortcut></Menubar.Item
        >
        <Menubar.Separator />
        <Menubar.Item onclick={() => handleEdit("cut")}
          >Cut<Menubar.Shortcut>{mod}+X</Menubar.Shortcut></Menubar.Item
        >
        <Menubar.Item onclick={() => handleEdit("copy")}
          >Copy<Menubar.Shortcut>{mod}+C</Menubar.Shortcut></Menubar.Item
        >
        <Menubar.Item onclick={() => handleEdit("paste")}
          >Paste<Menubar.Shortcut>{mod}+V</Menubar.Shortcut></Menubar.Item
        >
        <Menubar.Separator />
        <Menubar.CheckboxItem bind:checked={editorState.wordWrap}>
          <div class="flex items-center gap-2">
            Word Wrap
            {#if editorState.wordWrap}
              <Check class="ml-2 h-4 w-4" />
            {/if}
          </div>
          <Menubar.Shortcut>{wordWrapShortcut}</Menubar.Shortcut>
        </Menubar.CheckboxItem>
        <Menubar.Separator />
        <Menubar.Item onclick={() => handleEdit("selectAll")}
          >Select All<Menubar.Shortcut>{mod}+A</Menubar.Shortcut></Menubar.Item
        >
      </Menubar.Content>
    </Menubar.Menu>
  </Menubar.Root>
{/snippet}

<div
  class="relative flex h-10 w-full select-none items-center justify-between border-b bg-background shadow-sm"
>
  <div data-tauri-drag-region class="absolute inset-0 z-0"></div>

  {#if isMac}
    <!-- Mac Traffic Lights Space -->
    <!-- File & Edit menus are provided by the macOS native system menu bar -->
    <div
      class="group pointer-events-none z-10 flex h-full w-[72px] items-center justify-start gap-2 pl-4"
    ></div>

    <!-- Empty flex spacer so the traffic-lights side doesn't collapse -->
    <div class="pointer-events-none z-10 flex flex-1"></div>
  {:else}
    <!-- App Name + Menubar (Windows / Linux) -->
    <div class="pointer-events-none z-10 flex items-center pl-3">
      <span class="mr-2 text-xs font-semibold tracking-wide">Grayslate</span>
      {@render appMenubar()}
    </div>

    <!-- Window Controls (Windows / Linux) -->
    <div class="pointer-events-none z-10 flex h-full items-center">
      <button
        class="pointer-events-auto inline-flex h-full w-12 items-center justify-center text-foreground transition-colors hover:bg-foreground/10 hover:text-foreground focus:outline-none"
        onclick={() => appWindow.minimize()}
        aria-label="Minimize"
        title="Minimize"
      >
        <Minus class="h-4 w-4" />
      </button>

      <button
        class="pointer-events-auto inline-flex h-full w-12 items-center justify-center text-foreground transition-colors hover:bg-foreground/10 hover:text-foreground focus:outline-none"
        onclick={() => appWindow.toggleMaximize()}
        aria-label={isMaximized ? "Restore" : "Maximize"}
        title={isMaximized ? "Restore" : "Maximize"}
      >
        {#if isMaximized}
          <Minimize2 class="h-3.5 w-3.5" />
        {:else}
          <Square class="h-3.5 w-3.5" />
        {/if}
      </button>

      <button
        class="pointer-events-auto inline-flex h-full w-12 items-center justify-center text-foreground transition-colors hover:bg-[#c42b1c] hover:text-white focus:outline-none"
        onclick={() => appWindow.close()}
        aria-label="Close"
        title="Close"
      >
        <X class="h-4 w-4" />
      </button>
    </div>
  {/if}
</div>
