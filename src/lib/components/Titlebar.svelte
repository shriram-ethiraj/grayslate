<script lang="ts">
  import { Window } from "@tauri-apps/api/window";
  import { type } from "@tauri-apps/plugin-os";
  import { emit, listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";
  import * as Menubar from "$lib/components/ui/menubar/index.js";
  import Check from "~icons/lucide/check";
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
  import CodiconChromeRestore from "~icons/codicon/chrome-restore";
  import CodiconChromeMaximize from "~icons/codicon/chrome-maximize";
  import { registerHotkeys } from "$lib/hotkeys";
  import { formatForDisplay } from "@tanstack/hotkeys";

  let osType = $state("");
  const appWindow = new Window("main");

  let isMaximized = $state(false);
  let unlistenResize: (() => void) | undefined;

  const isMac = $derived(osType === "macos");
  const isLinux = $derived(osType === "linux");
  /** Redo shortcut differs between platforms */
  const redoShortcut = $derived(
    isMac ? formatForDisplay("Mod+Shift+Z") : formatForDisplay("Mod+Y"),
  );

  // Unlisten callback for the macOS native menu edit-action event.
  let unlistenEditAction: (() => void) | undefined;
  let unlistenWordWrap: (() => void) | undefined;

  // --- Linux / WebKitGTK first-click fix ---
  // WebKitGTK swallows the first pointerdown as a "focus the webview" event,
  // making menu triggers require two clicks to open. Pre-focusing the menubar
  // element tells GTK the webview is already focused, so it passes the click through.
  function handleMenubarPointerDown(e: PointerEvent) {
    if (!isLinux) return;
    const target = e.currentTarget as HTMLElement;
    if (!target.contains(document.activeElement)) {
      target.focus({ preventScroll: true });
    }
  }

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

  $effect(() => {
    return registerHotkeys([
      {
        key: "Mod+O",
        callback: (e) => {
          e.preventDefault();
          handleOpen();
        },
        options: { ignoreInputs: false },
      },
      {
        key: "Alt+Z",
        callback: (e) => {
          if (isMac) return;
          e.preventDefault();
          editorState.wordWrap = !editorState.wordWrap;
        },
        options: { ignoreInputs: false },
      },
    ]);
  });

  // Keep the macOS native menu bar checkmark in sync with editorState.wordWrap.
  // Runs whenever wordWrap or isMac changes; early-returns on non-macOS platforms.
  $effect(() => {
    if (!isMac) return;
    invoke("set_menu_word_wrap", { checked: editorState.wordWrap });
  });
</script>

{#snippet appMenubar()}
  <Menubar.Root
    class="pointer-events-auto border-none bg-transparent"
    onpointerdown={handleMenubarPointerDown}
  >
    <Menubar.Menu>
      <Menubar.Trigger class="cursor-pointer">File</Menubar.Trigger>
      <Menubar.Content>
        <Menubar.Item onclick={handleOpen}>
          Open File...
          <Menubar.Shortcut>{formatForDisplay("Mod+O")}</Menubar.Shortcut>
        </Menubar.Item>
      </Menubar.Content>
    </Menubar.Menu>
    <Menubar.Menu>
      <Menubar.Trigger class="cursor-pointer">Edit</Menubar.Trigger>
      <Menubar.Content>
        <Menubar.Item onclick={() => handleEdit("undo")}
          >Undo<Menubar.Shortcut>{formatForDisplay("Mod+Z")}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item onclick={() => handleEdit("redo")}
          >Redo<Menubar.Shortcut>{redoShortcut}</Menubar.Shortcut></Menubar.Item
        >
        <Menubar.Separator />
        <Menubar.Item onclick={() => handleEdit("cut")}
          >Cut<Menubar.Shortcut>{formatForDisplay("Mod+X")}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item onclick={() => handleEdit("copy")}
          >Copy<Menubar.Shortcut>{formatForDisplay("Mod+C")}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item onclick={() => handleEdit("paste")}
          >Paste<Menubar.Shortcut>{formatForDisplay("Mod+V")}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Separator />
        <Menubar.CheckboxItem bind:checked={editorState.wordWrap}>
          <div class="flex items-center gap-2">
            Word Wrap
            {#if editorState.wordWrap}
              <Check class="ml-2 h-4 w-4" />
            {/if}
          </div>
          <Menubar.Shortcut>{formatForDisplay("Alt+Z")}</Menubar.Shortcut>
        </Menubar.CheckboxItem>
        <Menubar.Separator />
        <Menubar.Item onclick={() => handleEdit("selectAll")}
          >Select All<Menubar.Shortcut
            >{formatForDisplay("Mod+A")}</Menubar.Shortcut
          ></Menubar.Item
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
      <img
        src="/logo.png"
        alt="Grayslate"
        class="mr-2 h-4 w-4 shrink-0"
        draggable="false"
      />
      {@render appMenubar()}
    </div>

    <!-- Window Controls (Windows / Linux) -->
    <div class="pointer-events-none z-10 flex h-full items-center">
      {#if isLinux}
        <div class="flex h-full items-center gap-1.5 pr-2">
          <button
            class="pointer-events-auto flex h-8 w-8 items-center justify-center rounded-full text-foreground transition-colors hover:bg-foreground/10 focus:outline-none"
            onclick={() => appWindow.minimize()}
            aria-label="Minimize"
            title="Minimize"
          >
            <Minus class="h-4 w-4" />
          </button>

          <button
            class="pointer-events-auto flex h-8 w-8 items-center justify-center rounded-full text-foreground transition-colors hover:bg-foreground/10 focus:outline-none"
            onclick={() => appWindow.toggleMaximize()}
            aria-label={isMaximized ? "Restore" : "Maximize"}
            title={isMaximized ? "Restore" : "Maximize"}
          >
            {#if isMaximized}
              <CodiconChromeRestore class="h-4.5 w-4.5" />
            {:else}
              <CodiconChromeMaximize class="h-3.5 w-3.5" />
            {/if}
          </button>

          <button
            class="pointer-events-auto flex h-8 w-8 items-center justify-center rounded-full text-foreground transition-colors hover:bg-foreground/10 focus:outline-none"
            onclick={() => appWindow.close()}
            aria-label="Close"
            title="Close"
          >
            <X class="h-4 w-4" />
          </button>
        </div>
      {:else}
        <button
          class="pointer-events-auto inline-flex h-full w-12 items-center justify-center text-foreground transition-colors hover:bg-foreground/10 focus:outline-none"
          onclick={() => appWindow.minimize()}
          aria-label="Minimize"
          title="Minimize"
        >
          <Minus class="h-4 w-4" />
        </button>

        <button
          class="pointer-events-auto inline-flex h-full w-12 items-center justify-center text-foreground transition-colors hover:bg-foreground/10 focus:outline-none"
          onclick={() => appWindow.toggleMaximize()}
          aria-label={isMaximized ? "Restore" : "Maximize"}
          title={isMaximized ? "Restore" : "Maximize"}
        >
          {#if isMaximized}
            <CodiconChromeRestore class="h-4.5 w-4.5" />
          {:else}
            <CodiconChromeMaximize class="h-3.5 w-3.5" />
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
      {/if}
    </div>
  {/if}
</div>
