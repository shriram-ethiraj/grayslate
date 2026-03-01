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
        showEditorLoader,
        updateEditorLoader,
        hideEditorLoader,
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

        // Stage 1: show loader, start at 0 %
        showEditorLoader("Opening file…", filename, 0);

        // Slowly animate the bar from 5 % → 65 % while the Rust read is in-flight.
        // The step shrinks each tick so it decelerates and never reaches 65 %
        // on its own — we'll snap it forward once the read returns.
        let simulated = 5;
        updateEditorLoader("Reading file…", filename, simulated);
        const ticker = setInterval(() => {
            const remaining = 65 - simulated;
            simulated += Math.max(0.3, remaining * 0.06);
            if (simulated >= 64.9) simulated = 64.9;
            updateEditorLoader("Reading file…", filename, simulated);
        }, 80);

        try {
            const content = await invoke<string>("read_file_content", {
                path: filePath,
            });

            // Stage 2: read complete
            clearInterval(ticker);
            updateEditorLoader("Detecting language…", filename, 72);

            // Detect language from filename + content; fall back to "text"
            await new Promise<void>((r) => setTimeout(r, 0)); // yield to let UI repaint
            const detected = languageDetector.detect(content, filename) ?? "text";

            // Stage 3: push into editor
            updateEditorLoader("Loading into editor…", filename, 88);
            await new Promise<void>((r) => setTimeout(r, 0));

            language = "auto";
            detectedLanguage = detected;
            value = content;

            // Stage 4: done
            updateEditorLoader("Done", "", 100);
            // Brief pause so the user sees 100 % before the overlay disappears
            await new Promise<void>((r) => setTimeout(r, 250));
        } catch (err: unknown) {
            clearInterval(ticker);
            const msg = typeof err === "string" ? err : "Failed to open file.";
            toast.error(msg);
        } finally {
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
