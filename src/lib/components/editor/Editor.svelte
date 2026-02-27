<script lang="ts">
    import { EditorState, Compartment } from "@codemirror/state";
    import { EditorView, basicSetup } from "codemirror";
    import { scrollPastEnd } from "@codemirror/view";
    import { createTheme } from "$lib/hooks/create-theme";
    import { andromedaConfig } from "$lib/themes/andromeda";
    import { materialLightConfig } from "$lib/themes/material-light";

    import { json } from "@codemirror/lang-json";
    import { javascript } from "@codemirror/lang-javascript";
    import { python } from "@codemirror/lang-python";
    import { html } from "@codemirror/lang-html";
    import { css } from "@codemirror/lang-css";
    import { yaml } from "@codemirror/lang-yaml";
    import { cpp } from "@codemirror/lang-cpp";
    import { java } from "@codemirror/lang-java";
    import { go } from "@codemirror/lang-go";
    import { xml } from "@codemirror/lang-xml";
    import { csv } from "codemirror-lang-csv";
    import { markdown } from "@codemirror/lang-markdown";
    import { jsonInlayHints } from "$lib/utils/editor/widgets/jsonInlayHints";
    import { jsonFoldWidget } from "$lib/utils/editor/widgets/jsonFoldWidget";
    import {
        jsonKeyPath,
        buildJsonPath,
        extractPropertyKey,
    } from "$lib/utils/editor/widgets/jsonKeyPath";
    import { colorHints } from "$lib/utils/editor/widgets/colorHints";
    import { markdownAutocompleteProvider } from "$lib/utils/editor/markdown/markdownAutocomplete";
    import { autocompletion } from "@codemirror/autocomplete";
    import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
    import { syntaxTree } from "@codemirror/language";
    import { toast } from "svelte-sonner";

    // Use Svelte 5 runes for the bound value
    let {
        value = $bindable(),
        line = $bindable(1),
        col = $bindable(1),
        selectionSize = $bindable(0),
        language = $bindable("text"),
        editorView = $bindable<EditorView | undefined>(undefined),
    } = $props();
    let view: EditorView;
    let themeCompartment: Compartment;
    let langCompartment: Compartment;

    let jsonContextMenuPath = $state("");
    let jsonContextMenuKey = $state("");
    let jsonContextMenuValue = $state("");

    function handleContextMenu(e: MouseEvent) {
        if (language !== "json" || !view) {
            e.stopPropagation();
            return;
        }

        const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
        if (!pos) {
            e.stopPropagation();
            return;
        }

        const tree = syntaxTree(view.state);
        let node = tree.resolveInner(pos, -1);

        let targetNode = node;
        if (["{", "}", "[", "]", ":", ","].includes(node.name) && node.parent) {
            targetNode = node.parent;
        }

        // Move the cursor to the clicked position without making a range
        // selection. A range selection would cause highlightSelectionMatches
        // (bundled in basicSetup) to underline every matching word in the
        // document — which is distracting when only the context menu is opening.
        // The copy values are derived from the syntax tree, not the selection,
        // so an empty (cursor-only) selection is sufficient here.
        view.dispatch({
            selection: { anchor: pos },
            scrollIntoView: false,
        });

        const validNames = [
            "PropertyName",
            "String",
            "Number",
            "Boolean",
            "Null",
            "Object",
            "Array",
            "Property",
        ];
        if (
            !validNames.includes(targetNode.name) &&
            targetNode.name !== "JsonText"
        ) {
            e.stopPropagation();
            return;
        }

        const path = buildJsonPath(view, pos, -1);
        if (!path || path === "$") {
            e.stopPropagation();
            return;
        }

        let valueToCopy = "";

        if (
            targetNode.name === "PropertyName" ||
            (targetNode.name === "Property" &&
                targetNode.firstChild?.name === "PropertyName")
        ) {
            let propNode =
                targetNode.name === "PropertyName"
                    ? targetNode.parent
                    : targetNode;
            if (propNode && propNode.name === "Property") {
                let valNode = propNode.lastChild;
                if (valNode) {
                    valueToCopy = view.state.sliceDoc(valNode.from, valNode.to);
                }
            }
        } else {
            valueToCopy = view.state.sliceDoc(targetNode.from, targetNode.to);
        }

        try {
            if (
                targetNode.name === "Object" ||
                targetNode.name === "Array" ||
                targetNode.name === "Property"
            ) {
                const parsed = JSON.parse(valueToCopy);
                valueToCopy = JSON.stringify(parsed, null, 2);
            } else if (
                targetNode.name === "String" ||
                targetNode.name === "PropertyName"
            ) {
                const parsed = JSON.parse(valueToCopy);
                if (typeof parsed === "string") {
                    valueToCopy = parsed;
                }
            } else {
                const parsed = JSON.parse(valueToCopy);
                if (typeof parsed !== "object") {
                    valueToCopy = String(parsed);
                }
            }
        } catch (err) {
            // keep raw sliceDoc value
        }

        // Extract the bare key name when right-clicking on a PropertyName node.
        let keyName = "";
        if (targetNode.name === "PropertyName") {
            keyName = extractPropertyKey(view, targetNode.parent!) ?? "";
        } else if (
            targetNode.name === "Property" ||
            (targetNode.name === "String" &&
                targetNode.parent?.name === "Property" &&
                targetNode.parent.firstChild === targetNode)
        ) {
            const propNode =
                targetNode.name === "Property"
                    ? targetNode
                    : targetNode.parent!;
            keyName = extractPropertyKey(view, propNode) ?? "";
        }

        jsonContextMenuPath = path;
        jsonContextMenuKey = keyName;
        jsonContextMenuValue = valueToCopy;
    }

    function getLanguageExtension(langId: string) {
        switch (langId) {
            case "json":
                return [json(), jsonInlayHints, jsonFoldWidget, jsonKeyPath];
            case "javascript":
                return javascript({ jsx: true });
            case "typescript":
                return javascript({ typescript: true, jsx: true });
            case "python":
                return python();
            case "html":
                return html();
            case "css":
                return css();
            case "yaml":
                return yaml();
            case "c":
                // cpp() covers both C and C++ syntax
                return cpp();
            case "cpp":
                return cpp();
            case "java":
                return java();
            case "go":
                return go();
            case "xml":
                return xml();
            case "csv":
                return csv();
            case "markdown":
                return [
                    markdown(),
                    autocompletion({
                        override: [markdownAutocompleteProvider],
                    }),
                ];
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
                colorHints,
                EditorView.domEventHandlers({
                    pointerdown(e) {
                        if (e.button === 2) {
                            // Prevent the browser's native right-click word-selection;
                            // handleContextMenu will make the explicit selection instead.
                            e.preventDefault();
                            return true;
                        }
                    },
                    mousedown(e) {
                        if (e.button === 2) {
                            e.preventDefault();
                            return true;
                        }
                    },
                }),
                // Listen for editor changes and sync back to the Svelte state
                EditorView.updateListener.of((update) => {
                    if (update.selectionSet || update.docChanged) {
                        const state = update.state;
                        const main = state.selection.main;
                        const lineInfo = state.doc.lineAt(main.head);
                        line = lineInfo.number;
                        col = main.head - lineInfo.from + 1;
                        // Sum character counts across all selection ranges
                        selectionSize = state.selection.ranges.reduce(
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

<ContextMenu.Root>
    <ContextMenu.Trigger class="h-full w-full block">
        <div
            class="editor-container"
            use:editor={value}
            oncontextmenu={handleContextMenu}
        ></div>
    </ContextMenu.Trigger>
    <ContextMenu.Content
        class="outline-none focus:outline-none focus-visible:outline-none"
    >
        <ContextMenu.Item
            onclick={() => {
                // Refocus the editor FIRST so that:
                //   a) the visual selection (and other-occurrence highlights)
                //      are restored immediately after the menu closes, and
                //   b) the editor's scroll position is not reset by the browser
                //      trying to scroll a newly-focused element into view.
                view?.focus();
                navigator.clipboard
                    .writeText(jsonContextMenuPath)
                    .then(() => toast.success("Copied path to clipboard"))
                    .catch(() => toast.error("Failed to copy path"));
            }}
        >
            Copy Path
        </ContextMenu.Item>
        {#if jsonContextMenuKey}
            <ContextMenu.Item
                onclick={() => {
                    view?.focus();
                    navigator.clipboard
                        .writeText(jsonContextMenuKey)
                        .then(() => toast.success("Copied key to clipboard"))
                        .catch(() => toast.error("Failed to copy key"));
                }}
            >
                Copy Key
            </ContextMenu.Item>
        {/if}
        <ContextMenu.Item
            onclick={() => {
                view?.focus();
                navigator.clipboard
                    .writeText(jsonContextMenuValue)
                    .then(() => toast.success("Copied value to clipboard"))
                    .catch(() => toast.error("Failed to copy value"));
            }}
        >
            Copy Value
        </ContextMenu.Item>
    </ContextMenu.Content>
</ContextMenu.Root>

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

    /* Hide cm-tooltip when context menu is open (the Trigger gets data-state="open") */
    :global([data-state="open"] .cm-tooltip) {
        display: none !important;
    }
</style>
