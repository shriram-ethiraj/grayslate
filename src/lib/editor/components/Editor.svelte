<script lang="ts">
    import { EditorState, Compartment } from "@codemirror/state";
    import { EditorView, basicSetup } from "codemirror";
    import { scrollPastEnd } from "@codemirror/view";
    import { createTheme } from "$lib/hooks/create-theme";
    import { andromedaConfig } from "$lib/themes/andromeda";
    import { materialLightConfig } from "$lib/themes/material-light";
    import { colorHints } from "$lib/editor/extensions/colorHints";
    import { getLanguageExtension } from "$lib/editor/config/languageExtensions";
    import { contextMenuExtension } from "$lib/editor/extensions/contextMenuExtension";
    import EditorContextMenu from "$lib/editor/components/EditorContextMenu.svelte";
    import { editorState } from "$lib/state/editor.svelte";

    let {
        value = $bindable(),
        line = $bindable(1),
        col = $bindable(1),
        selectionSize = $bindable(0),
        language = $bindable("text"),
        editorView = $bindable<EditorView | undefined>(undefined),
    } = $props();

    // $state so the value propagates reactively as a prop to JsonContextMenu.
    let view = $state<EditorView | undefined>(undefined);

    let themeCompartment: Compartment;
    let langCompartment: Compartment;
    let wordWrapCompartment: Compartment;

    // ---------------------------------------------------------------------------
    // Language compartment reconfiguration
    //
    // The initial language is already set inside EditorState.create (see the
    // `editor` action below).  We skip the very first effect run to avoid
    // immediately reconfiguring the compartment with freshly-created but
    // identical extension objects — which would be wasted work.
    // ---------------------------------------------------------------------------
    let langEffectInitialized = false;

    $effect(() => {
        const lang = language; // declare reactive dependency
        if (!langEffectInitialized) {
            langEffectInitialized = true;
            return;
        }
        if (view && langCompartment) {
            view.dispatch({
                effects: langCompartment.reconfigure(
                    getLanguageExtension(lang),
                ),
            });
        }
    });

    // ---------------------------------------------------------------------------
    // Word wrap compartment reconfiguration
    //
    // Toggling lineWrapping causes CodeMirror to reflow the entire document
    // (line heights change as lines wrap / unwrap). Without explicit scroll
    // anchoring the viewport jumps to an arbitrary position. We capture the
    // document position at the top of the viewport *before* the reconfigure,
    // then after the layout reflows we scroll that position back to the top.
    // ---------------------------------------------------------------------------
    let wrapEffectInitialized = false;

    $effect(() => {
        const wrap = editorState.wordWrap;
        if (!wrapEffectInitialized) {
            wrapEffectInitialized = true;
            return;
        }
        if (view && wordWrapCompartment) {
            // Capture the document position visible at the top of the viewport.
            const scrollTop = view.scrollDOM.scrollTop;
            const topBlock = view.lineBlockAtHeight(scrollTop);
            const topPos = topBlock.from;

            view.dispatch({
                effects: wordWrapCompartment.reconfigure(
                    wrap ? EditorView.lineWrapping : [],
                ),
            });

            // After layout reflows, scroll the same line back to the top.
            view.requestMeasure({
                read(v) {
                    return v.lineBlockAt(topPos).top;
                },
                write(newTop, v) {
                    v.scrollDOM.scrollTop = newTop;
                },
            });
        }
    });

    // ---------------------------------------------------------------------------
    // Svelte action — mounts and manages the CodeMirror instance
    // ---------------------------------------------------------------------------
    function editor(node: HTMLElement, initialValue: string) {
        themeCompartment = new Compartment();
        langCompartment = new Compartment();
        wordWrapCompartment = new Compartment();

        const isDark = document.documentElement.classList.contains("dark");
        const initialThemeExt = createTheme(
            isDark ? andromedaConfig : materialLightConfig,
        );

        const state = EditorState.create({
            doc: initialValue,
            extensions: [
                basicSetup,
                scrollPastEnd(),
                themeCompartment.of(initialThemeExt),
                langCompartment.of(getLanguageExtension(language)),
                wordWrapCompartment.of(
                    editorState.wordWrap ? EditorView.lineWrapping : [],
                ),
                colorHints,
                contextMenuExtension,
                // Sync cursor position, selection size, and document text back
                // to the parent Svelte component via bindable props.
                EditorView.updateListener.of((update) => {
                    if (update.selectionSet || update.docChanged) {
                        const s = update.state;
                        const main = s.selection.main;
                        const lineInfo = s.doc.lineAt(main.head);
                        line = lineInfo.number;
                        col = main.head - lineInfo.from + 1;
                        selectionSize = s.selection.ranges.reduce(
                            (sum, r) => sum + (r.to - r.from),
                            0,
                        );
                    }
                    if (update.docChanged) {
                        value = update.state.doc.toString();
                    }
                }),
            ],
        });

        const cmView = new EditorView({ state, parent: node });
        // Assign to both the local $state variable and the bindable prop so
        // both EditorContextMenu and external consumers receive the live view.
        view = cmView;
        editorView = cmView;
        editorState.activeView = cmView;

        // Watch for light/dark class toggling on <html> and swap the theme.
        // attributeFilter already guarantees only class mutations arrive, so
        // there is no need to re-check mutation.attributeName inside the callback.
        const observer = new MutationObserver(() => {
            const isDarkNow =
                document.documentElement.classList.contains("dark");
            const newTheme = createTheme(
                isDarkNow ? andromedaConfig : materialLightConfig,
            );
            cmView.dispatch({
                effects: themeCompartment.reconfigure(newTheme),
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
                    });
                }
            },
            destroy() {
                observer.disconnect();
                cmView.destroy();
                if (editorState.activeView === cmView) {
                    editorState.activeView = undefined;
                }
            },
        };
    }
</script>

<div class="editor-container" use:editor={value}></div>

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
