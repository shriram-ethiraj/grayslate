<script lang="ts">
    import Editor from "$lib/editor/components/Editor.svelte";
    import MarkdownPreview from "$lib/editor/components/markdown/MarkdownPreview.svelte";
    import CsvTableView from "./csv/CsvTableView.svelte";
    import StatusBar from "$lib/editor/components/StatusBar.svelte";
    import EditorLoader from "$lib/editor/components/EditorLoader.svelte";
    import { languageDetector } from "$lib/editor/core/languageDetector";
    import { debounce } from "lodash-es";
    import type { EditorView } from "codemirror";
    import { editorState, type FileType } from "$lib/state/editor.svelte";

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
