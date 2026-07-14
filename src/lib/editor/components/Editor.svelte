<script lang="ts">
  import { EditorView } from "codemirror";
  import { createTheme } from "$lib/hooks/create-theme";
  import { andromedaConfig } from "$lib/themes/andromeda";
  import { materialLightConfig } from "$lib/themes/material-light";
  import EditorContextMenu from "$lib/editor/components/EditorContextMenu.svelte";
  import FindReplace from "$lib/editor/components/FindReplace.svelte";
  import { editorState } from "$lib/state/editor.svelte";
  import { updateSearchStats } from "$lib/editor/core/actions";
  import {
    attachSessionBindings,
    captureManagedEditorView,
    createManagedEditorSession,
    detachSessionBindings,
    setManagedEditorFontSize,
    setManagedEditorIndent,
    ensureManagedEditorState,
    setManagedEditorLanguage,
    setManagedEditorWordWrap,
    DEFAULT_INDENT_CONFIG,
    type ManagedEditorSession,
  } from "$lib/editor/core/editorSession";

  let {
    value = $bindable(),
    documentLength = $bindable(0),
    lineCount = $bindable(1),
    line = $bindable(1),
    col = $bindable(1),
    selectionSize = $bindable(0),
    language = $bindable("text"),
    editorView = $bindable<EditorView | undefined>(undefined),
    session = createManagedEditorSession(),
    indentConfig = DEFAULT_INDENT_CONFIG,
  } = $props();

  // $state so the value propagates reactively as a prop to JsonContextMenu.
  let view = $state<EditorView | undefined>(undefined);

  // Guard flag: when syncBindings pushes a value update that originated
  // from the editor itself, there is no need for the Svelte action's
  // `update()` callback to compare the new string against `doc.toString()`
  // (a second O(n) serialization). The flag is set just before the
  // binding writes `value` and consumed by the action.
  let skipNextValueUpdate = false;

  $effect(() => {
    attachSessionBindings(session as ManagedEditorSession, {
      setValue: (nextValue) => {
        skipNextValueUpdate = true;
        value = nextValue;
      },
      setDocumentLength: (nextDocumentLength) => {
        documentLength = nextDocumentLength;
      },
      setLineCount: (nextLineCount) => {
        lineCount = nextLineCount;
      },
      setLine: (nextLine) => {
        line = nextLine;
      },
      setCol: (nextCol) => {
        col = nextCol;
      },
      setSelectionSize: (nextSelectionSize) => {
        selectionSize = nextSelectionSize;
      },
      onViewUpdate: (targetView, docChanged) => {
        updateSearchStats(targetView, { docChanged });
      },
    });

    return () => {
      detachSessionBindings(session as ManagedEditorSession);
    };
  });

  // ---------------------------------------------------------------------------
  // Language compartment reconfiguration
  //
  // We intentionally do NOT skip the first run (unlike the old boolean-guard
  // approach). On first mount the `use:editor` action runs synchronously and
  // sets `view`, so when this effect fires `view` is already available and
  // the correct language is applied immediately. This also handles the case
  // where the Editor is remounted (e.g. activeLanguage flips to "csv") while
  // reusing an existing session whose state still carries the old language.
  // ---------------------------------------------------------------------------
  $effect(() => {
    const lang = language;
    if (!view) return;
    setManagedEditorLanguage(session as ManagedEditorSession, lang);
  });

  // ---------------------------------------------------------------------------
  // Word wrap compartment reconfiguration
  //
  // Toggling lineWrapping causes CodeMirror to reflow the entire document
  // (line heights change as lines wrap / unwrap). Without explicit scroll
  // anchoring the viewport jumps to an arbitrary position. We capture the
  // document position at the top of the viewport *before* the reconfigure,
  // then after the layout reflows we scroll that position back to the top.
  //
  // Unlike the language effect we do NOT skip the first run — we need
  // `view` to be read so it becomes a tracked dependency.  On the very
  // first evaluation `view` is still `undefined` and the inner block is
  // skipped harmlessly; once the `editor` action assigns `view` the
  // effect re-runs and keeps the compartment in sync.
  // ---------------------------------------------------------------------------

  $effect(() => {
    const wrap = editorState.wordWrap;
    if (!view) return;
    setManagedEditorWordWrap(session as ManagedEditorSession, wrap);
  });

  $effect(() => {
    const fontSize = editorState.fontSize;
    if (!view) return;
    setManagedEditorFontSize(session as ManagedEditorSession, fontSize);
  });

  // ---------------------------------------------------------------------------
  // Indentation compartment reconfiguration
  //
  // Mirrors the word-wrap/font-size effects above: reads `indentConfig` (the
  // indentation picker's current mode/size) and pushes it into the session's
  // indentCompartment so Tab-key inserts and indentMore/indentLess reflect
  // the picker immediately, without remounting the editor.
  // ---------------------------------------------------------------------------
  $effect(() => {
    const config = indentConfig;
    if (!view) return;
    setManagedEditorIndent(session as ManagedEditorSession, config);
  });

  // ---------------------------------------------------------------------------
  // Svelte action — mounts and manages the CodeMirror instance
  // ---------------------------------------------------------------------------
  function editor(node: HTMLElement, initialValue: string) {
    const cmView = new EditorView({
      state: ensureManagedEditorState(
        session as ManagedEditorSession,
        initialValue,
        language,
      ),
      parent: node,
    });
    let observer: MutationObserver | undefined;

    function activateEditorSurface() {
      editorState.activeSurface = "editor";
      // Clear any DOM text selection from the markdown preview so the
      // two panes don't show simultaneous highlights side-by-side.
      const domSel = window.getSelection();
      if (domSel && !domSel.isCollapsed) {
        domSel.removeAllRanges();
      }
    }

    // Assign to both the local $state variable and the bindable prop so
    // both EditorContextMenu and external consumers receive the live view.
    (session as ManagedEditorSession).view = cmView;
    view = cmView;
    editorView = cmView;
    editorState.activeView = cmView;
    editorState.activeSurface = "editor";

    cmView.dom.addEventListener("pointerdown", activateEditorSurface, {
      passive: true,
    });
    cmView.dom.addEventListener("focusin", activateEditorSurface);
    cmView.dom.addEventListener("contextmenu", activateEditorSurface);

    // Watch for light/dark class toggling on <html> and swap the theme.
    // attributeFilter already guarantees only class mutations arrive, so
    // there is no need to re-check mutation.attributeName inside the callback.
    observer = new MutationObserver(() => {
      const isDarkNow = document.documentElement.classList.contains("dark");
      const editorSession = session as ManagedEditorSession;
      if (!editorSession.themeCompartment) return;
      const newTheme = createTheme(
        isDarkNow ? andromedaConfig : materialLightConfig,
      );
      cmView.dispatch({
        effects: editorSession.themeCompartment.reconfigure(newTheme),
      });
    });

    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });

    return {
      update(newValue: string) {
        // When the change originated from the editor (syncBindings →
        // setValue), the guard flag is already set and we can skip the
        // expensive doc.toString() comparison entirely.
        if (skipNextValueUpdate) {
          skipNextValueUpdate = false;
          return;
        }

        // External value change — patch the editor if the content
        // actually differs.
        if (newValue !== cmView.state.doc.toString()) {
          cmView.dispatch({
            changes: {
              from: 0,
              to: cmView.state.doc.length,
              insert: newValue,
            },
            effects: EditorView.scrollIntoView(0),
          });
        }
      },
      destroy() {
        observer?.disconnect();
        cmView.dom.removeEventListener("pointerdown", activateEditorSurface);
        cmView.dom.removeEventListener("focusin", activateEditorSurface);
        cmView.dom.removeEventListener("contextmenu", activateEditorSurface);
        captureManagedEditorView(session as ManagedEditorSession, cmView);
        cmView.destroy();
        if (editorState.activeView === cmView) {
          editorState.activeView = undefined;
        }
        if (editorState.activeSurface === "editor") {
          editorState.activeSurface = undefined;
        }
        view = undefined;
        editorView = undefined;
      },
    };
  }
</script>

<div class="editor-container" data-testid="editor" use:editor={value}>
  <FindReplace />
</div>

<!--
    EditorContextMenu listens on the CM DOM for contextmenu events.
    The companion contextMenuExtension does the basic focus shifting,
    and jsonContextMenuExtension (registered only for JSON) hit-tests JSON nodes.
    The Svelte component renders the unified floating menu.
-->
<EditorContextMenu {view} />

<style>
  .editor-container {
    width: 100%;
    height: 100%;
    overflow: hidden;
    overscroll-behavior: none;
  }

  /* CodeMirror must fill the container and match its scroll-behaviour */
  :global(.cm-editor) {
    height: 100%;
  }

  :global(.cm-scroller) {
    overscroll-behavior: none;
  }

  /* Line-number gutter width — mirrors VS Code's default */
  :global(.cm-editor .cm-lineNumbers .cm-gutterElement) {
    min-width: 40px !important;
    padding-right: 0px !important;
  }

  /* Hide CodeMirror tooltips while the context menu is open */
  :global(.editor-context-menu-open .cm-tooltip) {
    display: none !important;
  }
</style>
