<script lang="ts">
  import { Window } from "@tauri-apps/api/window";
  import { emit, listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";
  import * as Menubar from "$lib/components/ui/menubar/index.js";
  import Check from "~icons/lucide/check";
  import Minus from "~icons/lucide/minus";
  import X from "~icons/lucide/x";

  import { editorState } from "$lib/state/editor.svelte";
  import {
    decreaseEditorFontSize,
    increaseEditorFontSize,
    openFindReplacePanel as openEditorFindReplacePanel,
    openGoToLinePanel,
    openTransformationsPalette,
    resetEditorFontSize,
  } from "$lib/state/editor.svelte";
  import AboutDialog from "$lib/components/AboutDialog.svelte";
  import DeleteFileDialog from "$lib/components/DeleteFileDialog.svelte";
  import RenameFileDialog from "$lib/components/RenameFileDialog.svelte";
  import {
    checkForAppUpdates,
    openAboutDialog,
  } from "$lib/state/appMenu.svelte";
  import {
    editorUndo,
    editorRedo,
    editorCut,
    editorCopySelectionOrAll,
    editorPaste,
    editorSelectAll,
  } from "$lib/editor/core/actions";
  import {
    copyMarkdownPreviewSelectionOrAll,
    isMarkdownPreviewActive,
    selectAllMarkdownPreview,
  } from "$lib/editor/components/markdown/previewActions";
  import CodiconChromeRestore from "~icons/codicon/chrome-restore";
  import CodiconChromeMaximize from "~icons/codicon/chrome-maximize";
  import { registerHotkeys } from "$lib/hotkeys";
  import { formatForDisplay } from "@tanstack/hotkeys";
  import { platformState } from "$lib/state/platform.svelte";
  import { librarySidebarState } from "$lib/state/librarySidebar.svelte";

  const appWindow = new Window("main");

  let isMaximized = $state(false);
  let unlistenResize: (() => void) | undefined;

  const isMac = $derived(platformState.osType === "macos");
  const isLinux = $derived(platformState.osType === "linux");

  const displayName = $derived.by(() => {
    if (!editorState.currentFilePath) return "New Slate";
    const parts = editorState.currentFilePath.split(/[\\/]/);
    return parts[parts.length - 1] || "New Slate";
  });
  const showDirtyIndicator = $derived(
    editorState.isDirty && editorState.currentFileSource === "local",
  );
  /** Redo shortcut differs between platforms */
  const redoShortcut = $derived(
    isMac ? formatForDisplay("Mod+Shift+Z") : formatForDisplay("Mod+Y"),
  );
  const replaceShortcut = $derived(
    isMac ? formatForDisplay("Mod+Alt+F") : formatForDisplay("Mod+H"),
  );
  const increaseFontSizeShortcut = $derived(isMac ? "Cmd++" : "Ctrl++");
  const decreaseFontSizeShortcut = $derived(isMac ? "Cmd+-" : "Ctrl+-");
  const resetFontSizeShortcut = $derived(isMac ? "Cmd+0" : "Ctrl+0");

  // Unlisten callback for the macOS native menu edit-action event.
  let unlistenEditAction: (() => void) | undefined;
  let unlistenAbout: (() => void) | undefined;
  let unlistenCheckForUpdates: (() => void) | undefined;
  let unlistenWordWrap: (() => void) | undefined;
  let unlistenViewAction: (() => void) | undefined;

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
    // Track maximize state for Windows/Linux controls
    isMaximized = await appWindow.isMaximized();
    unlistenResize = await appWindow.onResized(async () => {
      isMaximized = await appWindow.isMaximized();
    });

    // On macOS, the in-window Menubar is hidden and the system menu bar is
    // used instead. Forward native menu edit events to the same handlers
    // used by the custom Menubar on Windows/Linux.
    if (platformState.osType === "macos") {
      unlistenAbout = await listen("menu://about", () => {
        void handleAbout();
      });

      unlistenCheckForUpdates = await listen("menu://check-for-updates", () => {
        void handleCheckForUpdates();
      });

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

      unlistenViewAction = await listen<string>("menu://view-action", (event) => {
        handleView(event.payload);
      });
    }
  });

  onDestroy(() => {
    unlistenAbout?.();
    unlistenCheckForUpdates?.();
    unlistenEditAction?.();
    unlistenWordWrap?.();
    unlistenViewAction?.();
    unlistenResize?.();
  });

  async function handleAbout() {
    await openAboutDialog();
  }

  async function handleCheckForUpdates() {
    await checkForAppUpdates({ openDialog: true });
  }

  async function handleNewFile() {
    await emit("menu://new-file");
  }

  async function handleOpen() {
    await emit("menu://open-file");
  }

  async function handleSave() {
    await emit("menu://save-file");
  }

  async function handleSaveAs() {
    await emit("menu://save-file-as");
  }

  async function handleEdit(action: string) {
    const view = editorState.activeView;
    const isCsvTableVisible =
      editorState.fileType === "csv" && editorState.csv.showTable;
    const isMarkdownPreviewVisible =
      editorState.fileType === "markdown" && editorState.markdown.showPreview;
    const markdownPreviewActive =
      isMarkdownPreviewVisible && isMarkdownPreviewActive();

    switch (action) {
      case "undo":
        if (markdownPreviewActive) return;
        if (isCsvTableVisible) {
          editorState.csv.undo?.();
        } else {
          if (!view) return;
          editorUndo(view, true);
        }
        break;
      case "redo":
        if (markdownPreviewActive) return;
        if (isCsvTableVisible) {
          editorState.csv.redo?.();
        } else {
          if (!view) return;
          editorRedo(view, true);
        }
        break;
      case "cut":
        if (markdownPreviewActive) return;
        if (isCsvTableVisible) return;
        if (!view) return;
        await editorCut(view);
        break;
      case "copy":
        if (markdownPreviewActive) {
          await copyMarkdownPreviewSelectionOrAll();
          return;
        }
        if (isCsvTableVisible) return;
        if (!view) return;
        await editorCopySelectionOrAll(view);
        break;
      case "paste":
        if (markdownPreviewActive) return;
        if (isCsvTableVisible) return;
        if (!view) return;
        await editorPaste(view);
        break;
      case "goToLine":
        if (isCsvTableVisible) return;
        openGoToLinePanel();
        break;
      case "find":
        if (isCsvTableVisible) return;
        openEditorFindReplacePanel(false);
        break;
      case "findFiles":
        librarySidebarState.requestActivateSearch?.();
        break;
      case "replace":
        if (isCsvTableVisible) return;
        openEditorFindReplacePanel(true);
        break;
      case "selectAll":
        if (markdownPreviewActive) {
          selectAllMarkdownPreview();
          return;
        }
        if (isCsvTableVisible) return;
        if (!view) return;
        editorSelectAll(view);
        break;
    }
  }

  function handleView(action: string) {
    switch (action) {
      case "increaseFontSize":
        increaseEditorFontSize();
        break;
      case "decreaseFontSize":
        decreaseEditorFontSize();
        break;
      case "resetFontSize":
        resetEditorFontSize();
        break;
    }
  }

  $effect(() => {
    return registerHotkeys([
      {
        key: "Mod+N",
        callback: (e) => {
          e.preventDefault();
          handleNewFile();
        },
        options: { ignoreInputs: false },
      },
      {
        key: "Mod+O",
        callback: (e) => {
          e.preventDefault();
          handleOpen();
        },
        options: { ignoreInputs: false },
      },
      {
        key: "Mod+S",
        callback: (e) => {
          e.preventDefault();
          handleSave();
        },
        options: { ignoreInputs: false },
      },
      {
        key: "Mod+Shift+S",
        callback: (e) => {
          e.preventDefault();
          handleSaveAs();
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
      {
        key: { key: "=", mod: true },
        callback: (e) => {
          e.preventDefault();
          handleView("increaseFontSize");
        },
        options: { ignoreInputs: false },
      },
      {
        key: { key: "=", mod: true, shift: true },
        callback: (e) => {
          e.preventDefault();
          handleView("increaseFontSize");
        },
        options: { ignoreInputs: false },
      },
      {
        key: "Mod+-",
        callback: (e) => {
          e.preventDefault();
          handleView("decreaseFontSize");
        },
        options: { ignoreInputs: false },
      },
      {
        key: "Mod+0",
        callback: (e) => {
          e.preventDefault();
          handleView("resetFontSize");
        },
        options: { ignoreInputs: false },
      },
      {
        key: "Mod+K",
        callback: (e) => {
          e.preventDefault();
          openTransformationsPalette();
        },
        options: { ignoreInputs: false },
      },
      {
        key: "Mod+Shift+P",
        callback: (e) => {
          e.preventDefault();
          openTransformationsPalette();
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

  // Keep the macOS native "Save" menu item enabled state in sync with isDirty.
  $effect(() => {
    if (!isMac) return;
    invoke("set_menu_save_enabled", { enabled: editorState.isDirty });
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
        <Menubar.Item onclick={handleNewFile}>
          New Slate
          <Menubar.Shortcut>{formatForDisplay("Mod+N")}</Menubar.Shortcut>
        </Menubar.Item>
        <Menubar.Separator />
        <Menubar.Item onclick={handleOpen}>
          Open File...
          <Menubar.Shortcut>{formatForDisplay("Mod+O")}</Menubar.Shortcut>
        </Menubar.Item>
        <Menubar.Separator />
        <Menubar.Item onclick={handleSave} disabled={!editorState.isDirty}>
          Save
          <Menubar.Shortcut>{formatForDisplay("Mod+S")}</Menubar.Shortcut>
        </Menubar.Item>
        <Menubar.Item onclick={handleSaveAs}>
          Save As...
          <Menubar.Shortcut>{formatForDisplay("Mod+Shift+S")}</Menubar.Shortcut>
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
        <Menubar.Item onclick={() => handleEdit("goToLine")}
          >Go To Line...<Menubar.Shortcut
            >{formatForDisplay("Mod+G")}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item onclick={() => handleEdit("find")}
          >Find...<Menubar.Shortcut>{formatForDisplay("Mod+F")}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item onclick={() => handleEdit("findFiles")}
          >Find Files...<Menubar.Shortcut
            >{formatForDisplay("Mod+P")}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item onclick={() => handleEdit("replace")}
          >Replace...<Menubar.Shortcut>{replaceShortcut}</Menubar.Shortcut
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
    <Menubar.Menu>
      <Menubar.Trigger class="cursor-pointer">View</Menubar.Trigger>
      <Menubar.Content>
        <Menubar.Item onclick={() => handleView("increaseFontSize")}
          >Increase Font Size<Menubar.Shortcut
            >{increaseFontSizeShortcut}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item onclick={() => handleView("decreaseFontSize")}
          >Decrease Font Size<Menubar.Shortcut
            >{decreaseFontSizeShortcut}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item onclick={() => handleView("resetFontSize")}
          >Reset Font Size<Menubar.Shortcut
            >{resetFontSizeShortcut}</Menubar.Shortcut
          ></Menubar.Item
        >
      </Menubar.Content>
    </Menubar.Menu>
    <Menubar.Menu>
      <Menubar.Trigger class="cursor-pointer">Help</Menubar.Trigger>
      <Menubar.Content>
        <Menubar.Item onclick={handleCheckForUpdates}
          >Check for Updates...</Menubar.Item
        >
        <Menubar.Separator />
        <Menubar.Item onclick={handleAbout}>About Grayslate</Menubar.Item>
      </Menubar.Content>
    </Menubar.Menu>
  </Menubar.Root>
{/snippet}

<div
  class="relative flex h-10 w-full select-none items-center justify-between border-b bg-background shadow-sm"
>
  <div data-tauri-drag-region class="absolute inset-0 z-0"></div>

  <!-- Centered file name: pointer-events-none so drag-region below remains active -->
  <div class="pointer-events-none absolute inset-0 z-5 flex items-center justify-center">
    <div class="relative max-w-[40%]">
      <span
        class="block truncate text-xs font-medium text-foreground pr-3"
        title={editorState.currentFilePath ?? displayName}
      >
        {displayName}
      </span>
      {#if showDirtyIndicator}
        <span class="absolute right-0 top-0 bottom-0 flex items-center text-xs font-medium">*</span>
      {/if}
    </div>
  </div>

  {#if isMac}
    <!-- Mac Traffic Lights Space -->
    <!-- File, Edit, and View menus are provided by the macOS native system menu bar -->
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

<AboutDialog />
<DeleteFileDialog />
<RenameFileDialog />
