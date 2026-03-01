<script lang="ts">
    import type { EditorView } from "codemirror";
    import { toast } from "svelte-sonner";
    import { tick } from "svelte";
    import {
        Copy as CopyIcon,
        Scissors,
        ClipboardPaste,
        KeyRound,
        Braces,
        Link,
        TextSelect,
    } from "@lucide/svelte";
    import {
        consumeContextMenuData,
        type ContextMenuData,
    } from "$lib/editor/extensions/contextMenuExtension";
    import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";

    let { view }: { view: EditorView | undefined } = $props();

    const isMac =
        typeof navigator !== "undefined" &&
        /Mac|iPhone|iPad|iPod/.test(navigator.userAgent);
    const modKey = isMac ? "⌘" : "Ctrl+";

    let open = $state(false);
    let menuX = $state(0);
    let menuY = $state(0);
    let menuRef = $state<HTMLDivElement | null>(null);
    let hasSelection = $state(false);
    let jsonData = $state<ContextMenuData | null>(null);
    let clipboardHasText = $state(false);

    /** Menu-item base classes (enabled state) */
    const itemBase =
        "relative flex w-full items-center rounded-sm px-2 py-1.5 text-sm outline-hidden select-none";
    const itemEnabled = `${itemBase} cursor-pointer hover:bg-accent hover:text-accent-foreground`;
    const itemDisabled = `${itemBase} opacity-40 cursor-default`;

    function openMenu(x: number, y: number) {
        open = true;
        menuX = x;
        menuY = y;

        if (view) {
            hasSelection = !view.state.selection.main.empty;
        }

        // Probe the OS clipboard so we can enable / disable Paste.
        // This runs async but resolves near-instantly in Tauri.
        readText()
            .then((text) => {
                clipboardHasText = text != null && text.length > 0;
            })
            .catch(() => {
                clipboardHasText = false;
            });

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
        jsonData = null;
        view?.focus();
    }

    // ── Listen on the CM DOM for contextmenu ────────────────────────────────
    $effect(() => {
        if (!view) return;
        const dom = view.dom;

        function onContextMenu(e: MouseEvent) {
            e.preventDefault(); // Always suppress native menu
            // If contextMenuExtension is active and hit a valid node, it stored data.
            jsonData = consumeContextMenuData();
            openMenu(e.clientX, e.clientY);
        }

        dom.addEventListener("contextmenu", onContextMenu);
        return () => dom.removeEventListener("contextmenu", onContextMenu);
    });

    // Toggle a class on the CM DOM so tooltips can be hidden via CSS
    $effect(() => {
        if (!view) return;
        if (open) {
            view.dom.classList.add("editor-context-menu-open");
        } else {
            view.dom.classList.remove("editor-context-menu-open");
        }
    });

    // ── Dismiss handlers ────────────────────────────────────────────────────
    $effect(() => {
        if (!open) return;

        function handlePointerDismiss(e: PointerEvent) {
            if (menuRef && menuRef.contains(e.target as Node)) return;
            // For right-clicks inside the editor, let the contextmenu listener handle reopening
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

    // ── Clipboard helpers ───────────────────────────────────────────────────
    function copyToClipboard(text: string, label: string) {
        close();
        writeText(text)
            .then(() => toast.success(`Copied ${label}`))
            .catch(() => toast.error(`Failed to copy ${label}`));
    }

    async function handleCut() {
        close();
        if (!view) return;
        const selection = view.state.selection.main;
        if (selection.empty) return;
        const text = view.state.sliceDoc(selection.from, selection.to);
        try {
            await writeText(text);
            view.dispatch({
                changes: { from: selection.from, to: selection.to, insert: "" },
            });
        } catch {
            toast.error("Failed to cut text");
        }
    }

    async function handleCopy() {
        close();
        if (!view) return;
        const selection = view.state.selection.main;
        if (selection.empty) return;
        const text = view.state.sliceDoc(selection.from, selection.to);
        try {
            await writeText(text);
        } catch {
            toast.error("Failed to copy text");
        }
    }

    async function handlePaste() {
        if (!clipboardHasText) return;
        close();
        if (!view) return;
        try {
            const text = await readText();
            if (text == null) return;
            const selection = view.state.selection.main;
            view.dispatch({
                changes: {
                    from: selection.from,
                    to: selection.to,
                    insert: text,
                },
                selection: { anchor: selection.from + text.length },
                scrollIntoView: true,
            });
        } catch {
            toast.error("Clipboard permission denied or failed to paste");
        }
    }

    function handleSelectAll() {
        close();
        if (!view) return;
        view.dispatch({
            selection: { anchor: 0, head: view.state.doc.length },
        });
    }
</script>

{#if open}
    <!-- svelte-ignore a11y_no_static_element_interactions a11y_interactive_supports_focus -->
    <div
        bind:this={menuRef}
        class="fixed z-50 min-w-44 flex flex-col gap-0.5 rounded-md border bg-popover p-1 text-popover-foreground shadow-md animate-in fade-in-0 zoom-in-95"
        style="left: {menuX}px; top: {menuY}px;"
        role="menu"
        tabindex="-1"
        oncontextmenu={(e) => e.preventDefault()}
    >
        <!-- ── JSON-specific options (always above generic items) ────────── -->
        {#if jsonData}
            {#if jsonData.path}
                <button
                    class={itemEnabled}
                    role="menuitem"
                    onclick={() => copyToClipboard(jsonData!.path!, "path")}
                >
                    <Link class="mr-2 h-4 w-4 shrink-0" />
                    <span>Copy Path</span>
                </button>
            {/if}

            {#if jsonData.key}
                <button
                    class={itemEnabled}
                    role="menuitem"
                    onclick={() => copyToClipboard(jsonData!.key!, "key")}
                >
                    <KeyRound class="mr-2 h-4 w-4 shrink-0" />
                    <span>Copy Key</span>
                </button>
            {/if}

            {#if jsonData.value}
                <button
                    class={itemEnabled}
                    role="menuitem"
                    onclick={() => copyToClipboard(jsonData!.value!, "value")}
                >
                    <Braces class="mr-2 h-4 w-4 shrink-0" />
                    <span>Copy Value</span>
                </button>
            {/if}

            <div class="my-1 h-px bg-muted"></div>
        {/if}

        <!-- ── Standard clipboard operations ─────────────────────────────── -->
        {#if hasSelection}
            <button class={itemEnabled} role="menuitem" onclick={handleCut}>
                <Scissors class="mr-2 h-4 w-4 shrink-0" />
                <span>Cut</span>
                <span class="ml-auto pl-4 text-xs text-muted-foreground"
                    >{modKey}X</span
                >
            </button>
            <button class={itemEnabled} role="menuitem" onclick={handleCopy}>
                <CopyIcon class="mr-2 h-4 w-4 shrink-0" />
                <span>Copy</span>
                <span class="ml-auto pl-4 text-xs text-muted-foreground"
                    >{modKey}C</span
                >
            </button>
        {/if}

        <button
            class={clipboardHasText ? itemEnabled : itemDisabled}
            role="menuitem"
            onclick={handlePaste}
        >
            <ClipboardPaste class="mr-2 h-4 w-4 shrink-0" />
            <span>Paste</span>
            <span class="ml-auto pl-4 text-xs text-muted-foreground"
                >{modKey}V</span
            >
        </button>

        <div class="my-1 h-px bg-muted"></div>

        <button class={itemEnabled} role="menuitem" onclick={handleSelectAll}>
            <TextSelect class="mr-2 h-4 w-4 shrink-0" />
            <span>Select All</span>
            <span class="ml-auto pl-4 text-xs text-muted-foreground"
                >{modKey}A</span
            >
        </button>
    </div>
{/if}
