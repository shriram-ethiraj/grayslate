<script lang="ts">
    import { EditorState } from "@codemirror/state";
    import { EditorView, basicSetup } from "codemirror";
    import { scrollPastEnd } from "@codemirror/view";
    import { createTheme } from "$lib/hooks/create-theme";
    import { andromedaConfig } from "$lib/themes/andromeda";
    import { materialLightConfig } from "$lib/themes/material-light";
    import { Compartment } from "@codemirror/state";

    import { json } from "@codemirror/lang-json";
    import { javascript } from "@codemirror/lang-javascript";
    import { python } from "@codemirror/lang-python";
    import { csv } from "codemirror-lang-csv";
    import { markdown } from "@codemirror/lang-markdown";
    import { jsonInlayHints } from "$lib/utils/editor/jsonInlayHints";

    // Use Svelte 5 runes for the bound value
    let {
        value = $bindable(),
        line = $bindable(1),
        col = $bindable(1),
        language = $bindable("text"),
        editorView = $bindable<EditorView | undefined>(undefined),
    } = $props();
    let view: EditorView;
    let themeCompartment: Compartment;
    let langCompartment: Compartment;

    function getLanguageExtension(langId: string) {
        switch (langId) {
            case "json":
                return [json(), jsonInlayHints];
            case "javascript":
                return javascript();
            case "python":
                return python();
            case "csv":
                return csv();
            case "markdown":
                return markdown();
            default:
                return [];
        }
    }

    $effect(() => {
        if (view && langCompartment) {
            view.dispatch({
                effects: langCompartment.reconfigure(
                    getLanguageExtension(language),
                ),
            });
        }
    });

    function editor(node: HTMLElement, initialValue: string) {
        // Create an extension compartment to dynamically swap themes
        themeCompartment = new Compartment();
        langCompartment = new Compartment();

        // Determine initial theme
        const isDark = document.documentElement.classList.contains("dark");
        const initialThemeExt = createTheme(
            isDark ? andromedaConfig : materialLightConfig,
        );

        let state = EditorState.create({
            doc: initialValue,
            extensions: [
                basicSetup,
                scrollPastEnd(),
                themeCompartment.of(initialThemeExt),
                langCompartment.of(getLanguageExtension(language)),
                // Listen for editor changes and sync back to the Svelte state
                EditorView.updateListener.of((update) => {
                    if (update.selectionSet || update.docChanged) {
                        const state = update.state;
                        const main = state.selection.main;
                        const lineInfo = state.doc.lineAt(main.head);
                        line = lineInfo.number;
                        col = main.head - lineInfo.from + 1;
                    }
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

        editorView = view;

        // Set up mutation observer to watch for theme changes on HTML
        const observer = new MutationObserver((mutations) => {
            mutations.forEach((mutation) => {
                if (mutation.attributeName === "class") {
                    const isDarkNow =
                        document.documentElement.classList.contains("dark");
                    const newTheme = createTheme(
                        isDarkNow ? andromedaConfig : materialLightConfig,
                    );
                    view?.dispatch({
                        effects: themeCompartment.reconfigure(newTheme),
                    });
                }
            });
        });

        observer.observe(document.documentElement, {
            attributes: true,
            attributeFilter: ["class"],
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
                observer.disconnect();
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
        overscroll-behavior: none;
    }

    /* Target the CodeMirror editor to fill the container and prevent rubber-banding */
    :global(.cm-editor) {
        height: 100%;
    }

    :global(.cm-scroller) {
        overscroll-behavior: none;
    }

    /* Style the line numbers column to resemble VS Code */
    :global(.cm-editor .cm-lineNumbers .cm-gutterElement) {
        min-width: 40px !important; /* Fixed width to accommodate at least 4 digits comfortably */
        padding-right: 0px !important;
    }
</style>
