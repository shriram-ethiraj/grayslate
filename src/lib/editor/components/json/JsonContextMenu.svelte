<script lang="ts">
    /**
     * JsonContextMenu.svelte
     *
     * Renders a floating context menu with JSON-aware "Copy Path / Key / Value"
     * actions.  Instead of wrapping the editor in a trigger element, this
     * component listens for `contextmenu` events on the CodeMirror DOM and
     * reads pre-computed data from the companion `jsonContextMenuExtension`
     * CM extension (registered only for JSON via `getLanguageExtension`).
     *
     * For non-JSON files the CM extension is absent, so no data is stored and
     * the native browser context menu is left untouched.
     *
     * Props:
     *   view – the live CodeMirror EditorView (may be undefined before mount)
     */
    import type { EditorView } from "codemirror";
    import {
        consumeJsonContextMenuData,
        type JsonContextMenuData,
    } from "$lib/editor/extensions/jsonContextMenu";
    import { toast } from "svelte-sonner";
    import { tick } from "svelte";

    let { view }: { view: EditorView | undefined } = $props();

    // ── Menu state ──────────────────────────────────────────────────────────
    let open = $state(false);
    let menuX = $state(0);
    let menuY = $state(0);
    let menuData = $state<JsonContextMenuData | null>(null);
    let menuRef = $state<HTMLDivElement | null>(null);

    // ── Open / close helpers ────────────────────────────────────────────────

    function openMenu(x: number, y: number, data: JsonContextMenuData) {
        menuData = data;
        menuX = x;
        menuY = y;
        open = true;

        // After Svelte renders the menu, clamp to viewport edges.
        tick().then(() => {
            if (!menuRef) return;
            const rect = menuRef.getBoundingClientRect();
            const vw = window.innerWidth;
            const vh = window.innerHeight;
            if (menuX + rect.width > vw) menuX = vw - rect.width - 4;
            if (menuY + rect.height > vh) menuY = vh - rect.height - 4;
        });
    }

    function close() {
        if (!open) return;
        open = false;
        menuData = null;
        view?.focus();
    }

    // ── Listen on the CM DOM for contextmenu ────────────────────────────────
    // The CM extension's handler fires first (registered on the same element
    // earlier), does hit-testing, and stores data in a module-level variable.
    // Our handler runs immediately after and reads it.

    $effect(() => {
        if (!view) return;
        const dom = view.dom;

        function onContextMenu(e: MouseEvent) {
            const data = consumeJsonContextMenuData();
            if (!data) {
                // Invalid target or no JSON extension — close any existing menu.
                // Don't preventDefault — let the native menu show for non-JSON.
                close();
                return;
            }
            e.preventDefault(); // suppress native menu — we show our own
            openMenu(e.clientX, e.clientY, data);
        }

        dom.addEventListener("contextmenu", onContextMenu);
        return () => dom.removeEventListener("contextmenu", onContextMenu);
    });

    // ── Toggle a class on the CM DOM so tooltips can be hidden via CSS ─────

    $effect(() => {
        if (!view) return;
        if (open) {
            view.dom.classList.add("json-context-menu-open");
        } else {
            view.dom.classList.remove("json-context-menu-open");
        }
    });

    // ── Dismiss handlers ────────────────────────────────────────────────────

    $effect(() => {
        if (!open) return;

        function handlePointerDismiss(e: PointerEvent) {
            if (menuRef && menuRef.contains(e.target as Node)) return;
            // For right-clicks inside the editor, the CM extension + our
            // contextmenu listener will decide whether to reopen the menu.
            // Don't close prematurely — it causes a visual flash.
            if (e.button === 2 && view?.dom.contains(e.target as Node)) return;
            close();
        }

        function handleKeydown(e: KeyboardEvent) {
            if (e.key === "Escape") {
                e.preventDefault();
                close();
            }
        }

        window.addEventListener("pointerdown", handlePointerDismiss);
        window.addEventListener("keydown", handleKeydown, true);

        return () => {
            window.removeEventListener("pointerdown", handlePointerDismiss);
            window.removeEventListener("keydown", handleKeydown, true);
        };
    });

    // ── Clipboard helper ────────────────────────────────────────────────────

    function copyToClipboard(text: string, label: string) {
        close();
        navigator.clipboard
            .writeText(text)
            .then(() => toast.success(`Copied ${label} to clipboard`))
            .catch(() => toast.error(`Failed to copy ${label}`));
    }
</script>

<!--
    Floating menu — rendered with position:fixed and portalled to the top-level
    DOM via Svelte's natural render flow (the component is a sibling of the
    editor, not a child of .cm-editor, so overflow/clipping is not an issue).
-->
{#if open && menuData}
    <!-- svelte-ignore a11y_no_static_element_interactions a11y_interactive_supports_focus -->
    <div
        bind:this={menuRef}
        class="fixed z-50 min-w-[8rem] rounded-md border bg-popover p-1 text-popover-foreground shadow-md animate-in fade-in-0 zoom-in-95"
        style="left: {menuX}px; top: {menuY}px;"
        role="menu"
        tabindex="-1"
        oncontextmenu={(e) => e.preventDefault()}
    >
        <button
            class="relative flex w-full cursor-pointer items-center rounded-sm px-2 py-1.5 text-sm outline-hidden select-none hover:bg-accent hover:text-accent-foreground"
            role="menuitem"
            onclick={() => copyToClipboard(menuData!.path, "path")}
        >
            Copy Path
        </button>

        {#if menuData.key}
            <button
                class="relative flex w-full cursor-pointer items-center rounded-sm px-2 py-1.5 text-sm outline-hidden select-none hover:bg-accent hover:text-accent-foreground"
                role="menuitem"
                onclick={() => copyToClipboard(menuData!.key, "key")}
            >
                Copy Key
            </button>
        {/if}

        <button
            class="relative flex w-full cursor-pointer items-center rounded-sm px-2 py-1.5 text-sm outline-hidden select-none hover:bg-accent hover:text-accent-foreground"
            role="menuitem"
            onclick={() => copyToClipboard(menuData!.value, "value")}
        >
            Copy Value
        </button>
    </div>
{/if}
