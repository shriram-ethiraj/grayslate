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
    ensureManagedEditorState,
    setManagedEditorLanguage,
    setManagedEditorWordWrap,
    type ManagedEditorSession,
  } from "$lib/editor/core/editorSession";

  let {
    value = $bindable(),
    line = $bindable(1),
    col = $bindable(1),
    selectionSize = $bindable(0),
    language = $bindable("text"),
    editorView = $bindable<EditorView | undefined>(undefined),
    session = createManagedEditorSession(),
  } = $props();

  // $state so the value propagates reactively as a prop to JsonContextMenu.
  let view = $state<EditorView | undefined>(undefined);

  $effect(() => {
    attachSessionBindings(session as ManagedEditorSession, {
      setValue: (nextValue) => {
        value = nextValue;
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
      onViewUpdate: (targetView) => {
        updateSearchStats(targetView);
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
    // Assign to both the local $state variable and the bindable prop so
    // both EditorContextMenu and external consumers receive the live view.
    (session as ManagedEditorSession).view = cmView;
    view = cmView;
    editorView = cmView;
    editorState.activeView = cmView;

    // Watch for light/dark class toggling on <html> and swap the theme.
    // attributeFilter already guarantees only class mutations arrive, so
    // there is no need to re-check mutation.attributeName inside the callback.
    const observer = new MutationObserver(() => {
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
        // Guard against infinite loops: only patch when the change
        // originated outside the editor (e.g. file load / undo).
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
        observer.disconnect();
        captureManagedEditorView(session as ManagedEditorSession, cmView);
        cmView.destroy();
        if (editorState.activeView === cmView) {
          editorState.activeView = undefined;
        }
        view = undefined;
        editorView = undefined;
      },
    };
  }
</script>

<div class="editor-container" use:editor={value}>
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
