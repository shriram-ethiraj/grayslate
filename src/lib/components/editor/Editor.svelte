<script lang="ts">
    import { EditorState } from "@codemirror/state";
    import { EditorView, basicSetup } from "codemirror";
    import { createTheme } from "$lib/hooks/create-theme";
    import { andromedaConfig } from "$lib/themes/andromeda";

    // Use Svelte 5 runes for the bound value
    let { value = $bindable() } = $props();
    let view: EditorView;

    function editor(node: HTMLElement, initialValue: string) {
        const themeExtension = createTheme(andromedaConfig);

        let state = EditorState.create({
            doc: initialValue,
            extensions: [
                basicSetup,
                themeExtension,
                // Listen for editor changes and sync back to the Svelte state
                EditorView.updateListener.of((update) => {
                    if (update.docChanged) {
                        value = update.state.doc.toString();
                    }
                }),
            ],
        });

        view = new EditorView({
            state,
            parent: node,
        });

        return {
            update(newValue: string) {
                // Prevent infinite loops if the change came from the editor itself
                if (newValue !== view.state.doc.toString()) {
                    view.dispatch({
                        changes: {
                            from: 0,
                            to: view.state.doc.length,
                            insert: newValue,
                        },
                    });
                }
            },
            destroy() {
                view.destroy();
            },
        };
    }
</script>

<div class="editor-container" use:editor={value}></div>

<style>
    .editor-container {
        width: 100%;
        height: 100%;
        overflow: hidden;
        overscroll-behavior-y: none;
    }

    /* Target the CodeMirror editor to fill the container and prevent rubber-banding */
    :global(.cm-editor) {
        height: 100%;
    }

    :global(.cm-scroller) {
        overscroll-behavior-y: none;
    }

    /* Style the line numbers column to resemble VS Code */
    :global(.cm-editor .cm-lineNumbers .cm-gutterElement) {
        min-width: 40px !important; /* Fixed width to accommodate at least 4 digits comfortably */
        padding-right: 0px !important;
    }
</style>
