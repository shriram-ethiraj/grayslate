<script lang="ts">
    /**
     * JsonContextMenu.svelte
     *
     * Wraps an arbitrary child element in a shadcn ContextMenu that exposes
     * JSON-aware "Copy Path / Key / Value" actions.  All context-menu state and
     * the hit-testing logic are self-contained here so Editor.svelte stays thin.
     *
     * Props:
     *   view     – the live CodeMirror EditorView (may be undefined before mount)
     *   language – the currently active language identifier
     *   children – snippet rendered inside the trigger
     */
    import type { EditorView } from "codemirror";
    import type { Snippet } from "svelte";
    import * as ContextMenu from "$lib/components/ui/context-menu/index.js";
    import { syntaxTree } from "@codemirror/language";
    import {
        buildJsonPath,
        extractPropertyKey,
    } from "$lib/editor/extensions/jsonKeyPath";
    import { toast } from "svelte-sonner";

    let {
        view,
        language,
        children,
    }: {
        view: EditorView | undefined;
        language: string;
        children: Snippet;
    } = $props();

    // Cached regex – compiled once per module load, not on every event.
    const WHITESPACE_RE = /\s/;

    let jsonContextMenuPath = $state("");
    let jsonContextMenuKey = $state("");
    let jsonContextMenuValue = $state("");

    // ---------------------------------------------------------------------------
    // Context-menu hit testing
    // ---------------------------------------------------------------------------

    export function handleContextMenu(e: MouseEvent) {
        if (language !== "json" || !view) {
            e.stopPropagation();
            return;
        }

        // posAtCoords returns null when the click lands outside any character
        // (e.g. gutter, empty padding below the last line).
        const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
        if (pos === null) {
            e.preventDefault();
            e.stopPropagation();
            return;
        }

        const tree = syntaxTree(view.state);

        // Suppress the menu for bare whitespace that isn't inside a JSON string.
        const charAtPos = view.state.sliceDoc(pos, pos + 1);
        if (!charAtPos || WHITESPACE_RE.test(charAtPos)) {
            const innerNode = tree.resolveInner(pos);
            if (
                innerNode.name !== "String" &&
                innerNode.name !== "PropertyName"
            ) {
                e.preventDefault();
                e.stopPropagation();
                return;
            }
            // Guard: pos must be inside the matched token's range.
            if (pos < innerNode.from || pos >= innerNode.to) {
                e.preventDefault();
                e.stopPropagation();
                return;
            }
        }

        let node = tree.resolveInner(pos, -1);

        // Structural punctuation → walk up to the containing node instead.
        let targetNode = node;
        if (["{", "}", "[", "]", ":", ","].includes(node.name) && node.parent) {
            targetNode = node.parent;
        }

        // Move cursor to click position without creating a range selection
        // (a range would trigger highlightSelectionMatches across the doc).
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
            e.preventDefault();
            e.stopPropagation();
            return;
        }

        const path = buildJsonPath(view, pos, -1);
        if (!path || path === "$") {
            e.stopPropagation();
            return;
        }

        // -----------------------------------------------------------------------
        // Resolve the raw value text from the syntax tree
        // -----------------------------------------------------------------------
        let valueToCopy = "";

        if (
            targetNode.name === "PropertyName" ||
            (targetNode.name === "Property" &&
                targetNode.firstChild?.name === "PropertyName")
        ) {
            const propNode =
                targetNode.name === "PropertyName"
                    ? targetNode.parent
                    : targetNode;
            if (propNode?.name === "Property") {
                const valNode = propNode.lastChild;
                if (valNode) {
                    valueToCopy = view.state.sliceDoc(valNode.from, valNode.to);
                }
            }
        } else {
            valueToCopy = view.state.sliceDoc(targetNode.from, targetNode.to);
        }

        // Pretty-print / unwrap the value for display in the clipboard.
        try {
            if (
                targetNode.name === "Object" ||
                targetNode.name === "Array" ||
                targetNode.name === "Property"
            ) {
                valueToCopy = JSON.stringify(JSON.parse(valueToCopy), null, 2);
            } else if (
                targetNode.name === "String" ||
                targetNode.name === "PropertyName"
            ) {
                const parsed = JSON.parse(valueToCopy);
                if (typeof parsed === "string") valueToCopy = parsed;
            } else {
                const parsed = JSON.parse(valueToCopy);
                if (typeof parsed !== "object") valueToCopy = String(parsed);
            }
        } catch {
            // keep raw sliceDoc text on parse failure
        }

        // -----------------------------------------------------------------------
        // Resolve the bare key name
        // -----------------------------------------------------------------------
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

    // ---------------------------------------------------------------------------
    // Clipboard helper
    // ---------------------------------------------------------------------------

    function copyToClipboard(text: string, label: string) {
        // Refocus FIRST so the visual selection is restored immediately and the
        // browser doesn't reset the editor's scroll position.
        view?.focus();
        navigator.clipboard
            .writeText(text)
            .then(() => toast.success(`Copied ${label} to clipboard`))
            .catch(() => toast.error(`Failed to copy ${label}`));
    }
</script>

<ContextMenu.Root
    onOpenChange={(open) => {
        // Restore editor focus when the menu closes for any reason.
        if (!open) view?.focus();
    }}
>
    <ContextMenu.Trigger class="h-full w-full block" oncontextmenu={handleContextMenu}>
        {@render children()}
    </ContextMenu.Trigger>

    <ContextMenu.Content
        class="outline-none focus:outline-none focus-visible:outline-none"
    >
        <ContextMenu.Item
            onclick={() => copyToClipboard(jsonContextMenuPath, "path")}
        >
            Copy Path
        </ContextMenu.Item>

        {#if jsonContextMenuKey}
            <ContextMenu.Item
                onclick={() => copyToClipboard(jsonContextMenuKey, "key")}
            >
                Copy Key
            </ContextMenu.Item>
        {/if}

        <ContextMenu.Item
            onclick={() => copyToClipboard(jsonContextMenuValue, "value")}
        >
            Copy Value
        </ContextMenu.Item>
    </ContextMenu.Content>
</ContextMenu.Root>
