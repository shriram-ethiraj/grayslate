<script lang="ts">
    import Editor from "$lib/editor/components/Editor.svelte";
    import MarkdownPreview from "$lib/editor/components/markdown/MarkdownPreview.svelte";
    import CsvTableView from "./csv/CsvTableView.svelte";
    import StatusBar from "$lib/editor/components/StatusBar.svelte";
    import EditorLoader from "$lib/editor/components/EditorLoader.svelte";
    import { languageDetector } from "$lib/editor/core/languageDetector";
    import { debounce } from "lodash-es";
    import type { EditorView } from "codemirror";
    import {
        editorState,
        updateEditorLoader,
        hideEditorLoader,
        startLoaderTicker,
        stopLoaderTicker,
        type FileType,
    } from "$lib/state/editor.svelte";
    import { listen } from "@tauri-apps/api/event";
    import { open as openFilePicker } from "@tauri-apps/plugin-dialog";
    import { invoke } from "@tauri-apps/api/core";
    import { toast } from "svelte-sonner";

    let value = $state("");
    let line = $state(1);
    let col = $state(1);
    let selectionSize = $state(0);
    let language = $state("auto");
    let detectedLanguage = $state("text");

    // Compute the actual language to apply to the editor
    let activeLanguage = $derived(
        language === "auto" ? detectedLanguage : language,
    );

    // Sync activeLanguage to global editorState
    $effect(() => {
        editorState.fileType = activeLanguage as FileType;
    });

    const checkLanguage = debounce((content: string) => {
        if (language === "auto" && content) {
            const result = languageDetector.detect(content);
            if (result) {
                detectedLanguage = result;
            }
        }
    }, 1000);

    // Watch the `value` and run language detection when it changes
    $effect(() => {
        checkLanguage(value);
    });

    let editorView = $state<EditorView | undefined>(undefined);

    // -----------------------------------------------------------------------
    // Menu: "File > Open File..."
    //
    // The Tauri menu emits "menu://open-file" when the user clicks the item
    // (or presses Ctrl/Cmd+O via its accelerator). We open a native file
    // picker, then invoke read_file_content on the Rust side which enforces
    // the 50 MB size limit before returning the text.
    // -----------------------------------------------------------------------
    async function openFile(): Promise<void> {
        const selected = await openFilePicker({
            multiple: false,
            directory: false,
        });

        // User cancelled the dialog
        if (!selected) return;

        const filePath = selected as string;
        const filename = filePath.replace(/\\/g, "/").split("/").pop() ?? "";

        // Start a decelerating progress ticker while the Rust read is in-flight
        startLoaderTicker("Reading file…", filename, {
            ceiling: 65,
            factor: 0.06,
            minStep: 0.3,
            interval: 80,
            startAt: 5,
        });

        try {
            const content = await invoke<string>("read_file_content", {
                path: filePath,
            });

            stopLoaderTicker();
            updateEditorLoader("Detecting language…", filename, 72);

            // Yield to let the UI repaint before running language detection
            await new Promise<void>((r) => setTimeout(r, 0));
            const detected = languageDetector.detect(content, filename) ?? "text";

            updateEditorLoader("Loading into editor…", filename, 88);
            await new Promise<void>((r) => setTimeout(r, 0));

            language = "auto";
            detectedLanguage = detected;
            value = content;
        } catch (err: unknown) {
            const msg = typeof err === "string" ? err : "Failed to open file.";
            toast.error(msg);
        } finally {
            // Always clean up — idempotent in the success path
            stopLoaderTicker();
            hideEditorLoader();
        }
    }

    // Register (and later clean up) the menu-event listener.
    $effect(() => {
        const unlisten = listen("menu://open-file", () => openFile());
        return () => {
            unlisten.then((fn) => fn());
        };
    });

    // Derive whether CSV table view is active
    let isCsvTableActive = $derived(
        activeLanguage === "csv" &&
            (editorState.csv.showTable || editorState.csv.serializing),
    );

    let csvInfo = $state({ rows: 0, cols: 0, delimiter: "", errors: 0 });
</script>

<div class="flex flex-1 flex-col w-full relative min-h-0 min-w-0 h-full">
    <div class="flex flex-1 min-h-0 min-w-0 relative">
        <EditorLoader
            visible={editorState.loader.visible}
            message={editorState.loader.message}
            subMessage={editorState.loader.subMessage}
            progress={editorState.loader.progress}
        />
        {#if isCsvTableActive}
            <!-- CSV Table View (replaces editor) -->
            <CsvTableView bind:content={value} bind:tableInfo={csvInfo} />
        {:else}
            <!-- Editor View -->
            <div
                class="flex-1 w-full min-w-0 {activeLanguage === 'markdown' &&
                editorState.markdown.showPreview
                    ? 'border-r border-border md:w-1/2 flex-none'
                    : ''}"
            >
                <Editor
                    bind:value
                    bind:line
                    bind:col
                    bind:selectionSize
                    language={activeLanguage}
                    bind:editorView
                />
            </div>

            <!-- Markdown Preview -->
            {#if activeLanguage === "markdown" && editorState.markdown.showPreview}
                <MarkdownPreview content={value} {editorView} />
            {/if}
        {/if}
    </div>
    <StatusBar
        {line}
        {col}
        {selectionSize}
        bind:language
        {detectedLanguage}
        {activeLanguage}
        {isCsvTableActive}
        {csvInfo}
    />
</div>
