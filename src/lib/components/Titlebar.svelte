<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "$lib/ipc";
  import { onMount, onDestroy, type Component } from "svelte";
  import * as Menubar from "$lib/components/ui/menubar/index.js";
  import { AppTooltip } from "$lib/components/ui/tooltip/index.js";
  import Check from "~icons/lucide/check";
  import Minus from "~icons/lucide/minus";
  import X from "~icons/lucide/x";
  import { toast } from "$lib/components/ui/sonner";
  import { notifyFileAlreadyOpen } from "$lib/files/recentFiles";

  import { editorState } from "$lib/state/editor.svelte";
  import {
    decreaseEditorFontSize,
    increaseEditorFontSize,
    openFindReplacePanel as openEditorFindReplacePanel,
    openGoToLinePanel,
    openTransformationsPalette,
    resetEditorFontSize,
    setEditorWordWrap,
  } from "$lib/state/editor.svelte";
  import {
    checkForAppUpdates,
    openAboutDialog,
  } from "$lib/state/appMenu.svelte";
  import {
    appDialogsState,
    openKeyboardShortcutsAppDialog,
    openSettingsAppDialog,
  } from "$lib/state/appDialogs.svelte";
  import { confirmBeforeLeavingDocument } from "$lib/state/unsavedChangesGuard.svelte";
  import {
    handleFocusedTextEdit,
    type FocusedTextEditAction,
  } from "$lib/editor/focusedTextEdit";
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
  import { beginTrackedWork, markE2EClosing, markE2EReady } from "virtual:grayslate-e2e-runtime";
  import {
    createBlankWindow,
    emitToCurrentWindow,
    pickDocumentIntoNewWindow,
  } from "$lib/windowing";

  const appWindow = getCurrentWindow();
  let editorActionsPromise: Promise<typeof import("$lib/editor/core/actions")> | undefined;
  let ActiveDialog = $state<Component | undefined>(undefined);
  let dialogLoadGeneration = 0;

  function loadEditorActions(): Promise<typeof import("$lib/editor/core/actions")> {
    if (!editorActionsPromise) {
      const finishTracking = beginTrackedWork("lazy-editor-actions-import");
      editorActionsPromise = import("$lib/editor/core/actions").finally(finishTracking);
    }
    return editorActionsPromise;
  }

  $effect(() => {
    const dialogType = appDialogsState.active.type;
    if (dialogType === "none") return;
    const generation = ++dialogLoadGeneration;
    const finishTracking = beginTrackedWork(`lazy-dialog-import:${dialogType}`);

    const loading: Promise<{ default: Component }> = (() => {
      switch (dialogType) {
        case "about": return import("$lib/components/AboutDialog.svelte");
        case "keyboard-shortcuts": return import("$lib/components/KeyboardShortcutsDialog.svelte");
        case "settings": return import("$lib/components/SettingsDialog.svelte");
        case "delete": return import("$lib/components/DeleteFileDialog.svelte");
        case "rename": return import("$lib/components/RenameFileDialog.svelte");
        case "unsaved-changes": return import("$lib/components/UnsavedChangesDialog.svelte");
      }
    })();

    void loading.then((module) => {
      if (generation === dialogLoadGeneration) ActiveDialog = module.default;
    }).finally(finishTracking);
  });

  let isMaximized = $state(false);
  let unlistenResize: (() => void) | undefined;
  let unlistenCloseRequested: (() => void) | undefined;

  const isMac = $derived(platformState.osType === "macos");
  const isLinux = $derived(platformState.osType === "linux");

  const displayName = $derived.by(() => {
    if (editorState.openingDocument?.fileName) {
      return editorState.openingDocument.fileName;
    }
    if (!editorState.currentFilePath) return "New Slate";
    const parts = editorState.currentFilePath.split(/[\\/]/);
    return parts[parts.length - 1] || "New Slate";
  });
  const displayPath = $derived(
    editorState.openingDocument?.path ?? editorState.currentFilePath ?? displayName,
  );
  const showDirtyIndicator = $derived(
    !editorState.openingDocument &&
      editorState.isDirty &&
      editorState.currentFileSource === "local",
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
  let unlistenSettings: (() => void) | undefined;
  let unlistenKeyboardShortcuts: (() => void) | undefined;
  let unlistenCheckForUpdates: (() => void) | undefined;
  let unlistenWordWrap: (() => void) | undefined;
  let unlistenViewAction: (() => void) | undefined;
  let unlistenNewWindow: (() => void) | undefined;
  let unlistenOpenFileNewWindow: (() => void) | undefined;

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

    // Guard window close when there are unsaved local-file changes.
    //
    // `preventDefault()` is unconditional: it only stops @tauri-apps/api from
    // calling `window.destroy()` itself, which would race the backend's
    // close-time slate flush and usually win, dropping the final save. Closing
    // is delegated to `prepare_close`, which flushes first and then destroys
    // the window from Rust. Returning early therefore leaves the window open.
    unlistenCloseRequested = await appWindow.onCloseRequested(async (event) => {
      event.preventDefault();

      const canClose = await confirmBeforeLeavingDocument();
      if (!canClose) {
        return;
      }

      // A failed flush deliberately keeps the window open so text is never
      // discarded because its selected encoding cannot be written.
      markE2EClosing();
      await invoke("prepare_close").catch((error: unknown) => {
        markE2EReady();
        console.error("Close: prepare_close failed:", error);
        toast.error(typeof error === "string"
          ? error
          : "Could not save this slate. The window was kept open.");
      });
    });

    // On macOS, the in-window Menubar is hidden and the system menu bar is
    // used instead. Forward native menu edit events to the same handlers
    // used by the custom Menubar on Windows/Linux.
    if (platformState.osType === "macos") {
      unlistenAbout = await appWindow.listen("menu://about", () => {
        void handleAbout();
      });

      unlistenSettings = await appWindow.listen("menu://settings", () => {
        handleSettings();
      });

      unlistenKeyboardShortcuts = await appWindow.listen("menu://keyboard-shortcuts", () => {
        handleKeyboardShortcuts();
      });

      unlistenCheckForUpdates = await appWindow.listen("menu://check-for-updates", () => {
        void handleCheckForUpdates();
      });

      unlistenNewWindow = await appWindow.listen("menu://new-window", () => {
        void handleNewWindow();
      });

      unlistenOpenFileNewWindow = await appWindow.listen("menu://open-file-new-window", () => {
        void handleOpenInNewWindow();
      });

      unlistenEditAction = await appWindow.listen<EditAction>(
        "menu://edit-action",
        (event) => {
          handleEdit(event.payload);
        },
      );

      // Word wrap uses a separate event that carries the actual
      // checked boolean from the native CheckMenuItem.  Setting
      // the state directly (instead of toggling) keeps the two
      // sides in lock-step regardless of muda's auto-toggle.
      unlistenWordWrap = await appWindow.listen<boolean>(
        "menu://word-wrap-state",
        (event) => {
          setEditorWordWrap(event.payload);
        },
      );

      unlistenViewAction = await appWindow.listen<string>("menu://view-action", (event) => {
        handleView(event.payload);
      });
    }
  });

  onDestroy(() => {
    unlistenAbout?.();
    unlistenSettings?.();
    unlistenKeyboardShortcuts?.();
    unlistenCheckForUpdates?.();
    unlistenEditAction?.();
    unlistenWordWrap?.();
    unlistenViewAction?.();
    unlistenNewWindow?.();
    unlistenOpenFileNewWindow?.();
    unlistenResize?.();
    unlistenCloseRequested?.();
  });

  async function handleAbout() {
    await openAboutDialog();
  }

  function handleSettings() {
    openSettingsAppDialog();
  }

  function handleKeyboardShortcuts() {
    openKeyboardShortcutsAppDialog();
  }

  async function handleCheckForUpdates() {
    await checkForAppUpdates({ openDialog: true });
  }

  async function handleNewFile() {
    await emitToCurrentWindow("menu://new-file");
  }

  async function handleNewWindow() {
    try {
      await createBlankWindow();
    } catch (error) {
      toast.error(typeof error === "string" ? error : "Could not create a new window.");
    }
  }

  async function handleOpen() {
    await emitToCurrentWindow("menu://open-file");
  }

  async function handleOpenInNewWindow() {
    try {
      const picked = await pickDocumentIntoNewWindow();
      if (
        picked?.result.kind === "focused-existing" &&
        (
          editorState.currentDocumentId === picked.document.documentId ||
          editorState.currentFilePath === picked.document.displayPath
        )
      ) {
        notifyFileAlreadyOpen(picked.document.fileName || picked.document.displayPath);
      }
    } catch (error) {
      toast.error(typeof error === "string" ? error : "Could not open the file in a new window.");
    }
  }

  async function handleSave() {
    await emitToCurrentWindow("menu://save-file");
  }

  async function handleSaveAs() {
    await emitToCurrentWindow("menu://save-file-as");
  }

  type EditAction =
    | FocusedTextEditAction
    | "goToLine"
    | "find"
    | "findFiles"
    | "replace";

  async function handleEdit(action: EditAction) {
    const view = editorState.activeView;
    const isCsvTableVisible =
      editorState.fileType === "csv" && editorState.csv.showTable;
    const isMarkdownPreviewVisible =
      editorState.fileType === "markdown" && editorState.markdown.showPreview;
    const markdownPreviewActive =
      isMarkdownPreviewVisible && isMarkdownPreviewActive();

    const activeElement = document.activeElement;
    const csvEditInputOwnsCopy =
      action === "copy" &&
      isCsvTableVisible &&
      activeElement instanceof HTMLElement &&
      activeElement.closest('[data-testid="csv-table"]') !== null;

    if (
      !csvEditInputOwnsCopy &&
      (action === "undo" ||
        action === "redo" ||
        action === "cut" ||
        action === "copy" ||
        action === "selectAll") &&
      handleFocusedTextEdit(action, view?.dom)
    ) {
      return;
    }

    switch (action) {
      case "undo":
        if (markdownPreviewActive) return;
        if (isCsvTableVisible) {
          editorState.csv.undo?.();
        } else {
          if (!view) return;
          const { editorUndo } = await loadEditorActions();
          editorUndo(view, true);
        }
        break;
      case "redo":
        if (markdownPreviewActive) return;
        if (isCsvTableVisible) {
          editorState.csv.redo?.();
        } else {
          if (!view) return;
          const { editorRedo } = await loadEditorActions();
          editorRedo(view, true);
        }
        break;
      case "cut":
        if (markdownPreviewActive) return;
        if (isCsvTableVisible) return;
        if (!view) return;
        const { editorCut } = await loadEditorActions();
        await editorCut(view);
        break;
      case "copy":
        if (editorState.copyInProgress) return;
        if (markdownPreviewActive) {
          await copyMarkdownPreviewSelectionOrAll();
          return;
        }
        if (isCsvTableVisible) {
          await editorState.csv.copy?.();
          return;
        }
        if (!view) return;
        const { editorCopySelectionOrAll } = await loadEditorActions();
        await editorCopySelectionOrAll(view);
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
        const { editorSelectAll } = await loadEditorActions();
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

  function handleLinuxFocusedTextShortcut(
    event: KeyboardEvent,
    action: "undo" | "redo",
  ): void {
    if (!handleFocusedTextEdit(action, editorState.activeView?.dom)) return;

    // WebKitGTK does not reliably apply its native input history command for
    // Ctrl+Z / Ctrl+Y. Claim the shortcut only when a regular text control
    // owns focus; CodeMirror and CSV table history continue through their own
    // scoped keymaps.
    event.preventDefault();
    event.stopPropagation();
  }

  $effect(() => {
    if (!isLinux) return;

    return registerHotkeys([
      {
        key: "Mod+Z",
        callback: (event) => handleLinuxFocusedTextShortcut(event, "undo"),
        options: {
          ignoreInputs: false,
          preventDefault: false,
          stopPropagation: false,
        },
      },
      {
        key: "Mod+Shift+Z",
        callback: (event) => handleLinuxFocusedTextShortcut(event, "redo"),
        options: {
          ignoreInputs: false,
          preventDefault: false,
          stopPropagation: false,
        },
      },
      {
        key: "Mod+Y",
        callback: (event) => handleLinuxFocusedTextShortcut(event, "redo"),
        options: {
          ignoreInputs: false,
          preventDefault: false,
          stopPropagation: false,
        },
      },
    ]);
  });

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
        key: "Mod+Shift+N",
        callback: (e) => {
          e.preventDefault();
          void handleNewWindow();
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
        // On macOS the native menu item owns Cmd+, — this handles Win/Linux.
        key: "Mod+,",
        callback: (e) => {
          if (isMac) return;
          e.preventDefault();
          handleSettings();
        },
        options: { ignoreInputs: false },
      },
      {
        key: "Alt+Z",
        callback: (e) => {
          if (isMac) return;
          e.preventDefault();
          setEditorWordWrap(!editorState.wordWrap);
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

  // Keep the OS taskbar/Dock/Alt-Tab title synchronized without granting the
  // webview unrestricted core window-title permissions.
  $effect(() => {
    const openingDocument = editorState.openingDocument;
    void invoke("sync_window_title", {
      documentId: openingDocument?.documentId ?? editorState.currentDocumentId ?? null,
      documentGeneration:
        openingDocument?.documentGeneration ?? editorState.currentDocumentGeneration ?? null,
      dirty: openingDocument ? false : editorState.isDirty,
    });
  });

  // Keep the macOS native "Save" menu item enabled state in sync with isDirty.
  $effect(() => {
    if (!isMac) return;
    invoke("set_menu_save_enabled", {
      enabled: editorState.isDirty && !editorState.saveInProgress,
    });
  });
</script>

{#snippet appMenubar()}
  <Menubar.Root
    class="pointer-events-auto border-none bg-transparent"
    onpointerdown={handleMenubarPointerDown}
  >
    <Menubar.Menu>
      <Menubar.Trigger data-testid="menu-file" class="cursor-pointer">File</Menubar.Trigger>
      <Menubar.Content>
        <Menubar.Item data-testid="menu-new-slate" onclick={handleNewFile}>
          New Slate
          <Menubar.Shortcut>{formatForDisplay("Mod+N")}</Menubar.Shortcut>
        </Menubar.Item>
        <Menubar.Item data-testid="menu-new-window" onclick={handleNewWindow}>
          New Window
          <Menubar.Shortcut>{formatForDisplay("Mod+Shift+N")}</Menubar.Shortcut>
        </Menubar.Item>
        <Menubar.Separator />
        <Menubar.Item data-testid="menu-open-file" onclick={handleOpen}>
          Open File...
          <Menubar.Shortcut>{formatForDisplay("Mod+O")}</Menubar.Shortcut>
        </Menubar.Item>
        <Menubar.Item data-testid="menu-open-file-new-window" onclick={handleOpenInNewWindow}>
          Open File in New Window...
        </Menubar.Item>
        <Menubar.Separator />
        <Menubar.Item
          data-testid="menu-save"
          onclick={handleSave}
          disabled={!editorState.isDirty || editorState.saveInProgress}
        >
          Save
          <Menubar.Shortcut>{formatForDisplay("Mod+S")}</Menubar.Shortcut>
        </Menubar.Item>
        <Menubar.Item
          data-testid="menu-save-as"
          onclick={handleSaveAs}
          disabled={editorState.saveInProgress}
        >
          Save As...
          <Menubar.Shortcut>{formatForDisplay("Mod+Shift+S")}</Menubar.Shortcut>
        </Menubar.Item>
        <Menubar.Separator />
        <Menubar.Item data-testid="menu-settings" onclick={handleSettings}>
          Settings...
          <Menubar.Shortcut>{formatForDisplay("Mod+,")}</Menubar.Shortcut>
        </Menubar.Item>
      </Menubar.Content>
    </Menubar.Menu>
    <Menubar.Menu>
      <Menubar.Trigger data-testid="menu-edit" class="cursor-pointer">Edit</Menubar.Trigger>
      <Menubar.Content>
        <Menubar.Item data-testid="menu-undo" onclick={() => handleEdit("undo")}
          >Undo<Menubar.Shortcut>{formatForDisplay("Mod+Z")}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item data-testid="menu-redo" onclick={() => handleEdit("redo")}
          >Redo<Menubar.Shortcut>{redoShortcut}</Menubar.Shortcut></Menubar.Item
        >
        <Menubar.Separator />
        <Menubar.Item data-testid="menu-cut" onclick={() => handleEdit("cut")}
          >Cut<Menubar.Shortcut>{formatForDisplay("Mod+X")}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item
          data-testid="menu-copy"
          disabled={editorState.copyInProgress}
          onclick={() => handleEdit("copy")}
          >Copy<Menubar.Shortcut>{formatForDisplay("Mod+C")}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Separator />
        <Menubar.Item data-testid="menu-go-to-line" onclick={() => handleEdit("goToLine")}
          >Go To Line...<Menubar.Shortcut
            >{formatForDisplay("Mod+G")}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item data-testid="menu-find" onclick={() => handleEdit("find")}
          >Find...<Menubar.Shortcut>{formatForDisplay("Mod+F")}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item data-testid="menu-find-files" onclick={() => handleEdit("findFiles")}
          >Find Files...<Menubar.Shortcut
            >{formatForDisplay("Mod+P")}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item data-testid="menu-replace" onclick={() => handleEdit("replace")}
          >Replace...<Menubar.Shortcut>{replaceShortcut}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Separator />
        <Menubar.CheckboxItem
          data-testid="menu-word-wrap"
          checked={editorState.wordWrap}
          onCheckedChange={(checked) => setEditorWordWrap(checked)}
        >
          <div class="flex items-center gap-2">
            Word Wrap
            {#if editorState.wordWrap}
              <Check class="ml-2 h-4 w-4" />
            {/if}
          </div>
          <Menubar.Shortcut>{formatForDisplay("Alt+Z")}</Menubar.Shortcut>
        </Menubar.CheckboxItem>
        <Menubar.Separator />
        <Menubar.Item data-testid="menu-select-all" onclick={() => handleEdit("selectAll")}
          >Select All<Menubar.Shortcut
            >{formatForDisplay("Mod+A")}</Menubar.Shortcut
          ></Menubar.Item
        >
      </Menubar.Content>
    </Menubar.Menu>
    <Menubar.Menu>
      <Menubar.Trigger data-testid="menu-view" class="cursor-pointer">View</Menubar.Trigger>
      <Menubar.Content>
        <Menubar.Item data-testid="menu-increase-font" onclick={() => handleView("increaseFontSize")}
          >Increase Font Size<Menubar.Shortcut
            >{increaseFontSizeShortcut}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item data-testid="menu-decrease-font" onclick={() => handleView("decreaseFontSize")}
          >Decrease Font Size<Menubar.Shortcut
            >{decreaseFontSizeShortcut}</Menubar.Shortcut
          ></Menubar.Item
        >
        <Menubar.Item data-testid="menu-reset-font" onclick={() => handleView("resetFontSize")}
          >Reset Font Size<Menubar.Shortcut
            >{resetFontSizeShortcut}</Menubar.Shortcut
          ></Menubar.Item
        >
      </Menubar.Content>
    </Menubar.Menu>
    <Menubar.Menu>
      <Menubar.Trigger data-testid="app-help-menu" class="cursor-pointer">Help</Menubar.Trigger>
      <Menubar.Content>
        <Menubar.Item data-testid="help-keyboard-shortcuts" onclick={handleKeyboardShortcuts}
          >Keyboard Shortcuts...</Menubar.Item
        >
        <Menubar.Separator />
        <Menubar.Item data-testid="menu-check-updates" onclick={handleCheckForUpdates}
          >Check for Updates...</Menubar.Item
        >
        <Menubar.Separator />
        <Menubar.Item data-testid="menu-about" onclick={handleAbout}>About Grayslate</Menubar.Item>
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
      <AppTooltip content={displayPath} side="bottom">
        {#snippet trigger({ props })}
          <span
            {...props}
            data-tauri-drag-region
            data-testid="title-file-name"
            data-document-path={displayPath}
            class="pointer-events-auto block truncate pr-3 text-xs font-medium text-foreground"
          >
            {displayName}
          </span>
        {/snippet}
      </AppTooltip>
      {#if showDirtyIndicator}
        <span data-testid="title-dirty-indicator" class="absolute right-0 top-0 bottom-0 flex items-center text-xs font-medium">*</span>
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
          <AppTooltip content="Minimize" side="bottom">
            {#snippet trigger({ props })}
              <button
                {...props}
                class="pointer-events-auto ui-state flex h-8 w-8 items-center justify-center rounded-full text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                onclick={() => appWindow.minimize()}
                aria-label="Minimize"
                data-testid="window-minimize"
              >
                <Minus class="h-4 w-4" />
              </button>
            {/snippet}
          </AppTooltip>

          <AppTooltip content={isMaximized ? "Restore" : "Maximize"} side="bottom">
            {#snippet trigger({ props })}
              <button
                {...props}
                class="pointer-events-auto ui-state flex h-8 w-8 items-center justify-center rounded-full text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                onclick={() => appWindow.toggleMaximize()}
                aria-label={isMaximized ? "Restore" : "Maximize"}
                data-testid="window-maximize"
                data-maximized={isMaximized ? "true" : "false"}
              >
                {#if isMaximized}
                  <CodiconChromeRestore class="h-4.5 w-4.5" />
                {:else}
                  <CodiconChromeMaximize class="h-3.5 w-3.5" />
                {/if}
              </button>
            {/snippet}
          </AppTooltip>

          <AppTooltip content="Close" side="bottom">
            {#snippet trigger({ props })}
              <button
                {...props}
                class="pointer-events-auto ui-state flex h-8 w-8 items-center justify-center rounded-full text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                onclick={() => appWindow.close()}
                aria-label="Close"
                data-testid="window-close"
              >
                <X class="h-4 w-4" />
              </button>
            {/snippet}
          </AppTooltip>
        </div>
      {:else}
        <AppTooltip content="Minimize" side="bottom">
          {#snippet trigger({ props })}
            <button
              {...props}
              class="pointer-events-auto ui-state inline-flex h-full w-12 items-center justify-center text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onclick={() => appWindow.minimize()}
              aria-label="Minimize"
              data-testid="window-minimize"
            >
              <Minus class="h-4 w-4" />
            </button>
          {/snippet}
        </AppTooltip>

        <AppTooltip content={isMaximized ? "Restore" : "Maximize"} side="bottom">
          {#snippet trigger({ props })}
            <button
              {...props}
              class="pointer-events-auto ui-state inline-flex h-full w-12 items-center justify-center text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onclick={() => appWindow.toggleMaximize()}
              aria-label={isMaximized ? "Restore" : "Maximize"}
              data-testid="window-maximize"
              data-maximized={isMaximized ? "true" : "false"}
            >
              {#if isMaximized}
                <CodiconChromeRestore class="h-4.5 w-4.5" />
              {:else}
                <CodiconChromeMaximize class="h-3.5 w-3.5" />
              {/if}
            </button>
          {/snippet}
        </AppTooltip>

        <AppTooltip content="Close" side="bottom">
          {#snippet trigger({ props })}
            <button
              {...props}
              class="pointer-events-auto inline-flex h-full w-12 items-center justify-center text-foreground transition-colors hover:bg-[#c42b1c] hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onclick={() => appWindow.close()}
              aria-label="Close"
              data-testid="window-close"
            >
              <X class="h-4 w-4" />
            </button>
          {/snippet}
        </AppTooltip>
      {/if}
    </div>
  {/if}
</div>

{#if ActiveDialog}
  <ActiveDialog />
{/if}
