<script lang="ts">
    import Editor from "$lib/components/editor/Editor.svelte";
    import MarkdownPreview from "$lib/components/editor/MarkdownPreview.svelte";
    import StatusBar from "$lib/components/editor/StatusBar.svelte";
    import { languageDetector } from "$lib/utils/language";
    import { debounce } from "lodash-es";
    import type { EditorView } from "codemirror";

    let value = $state("");
    let line = $state(1);
    let col = $state(1);
    let language = $state("auto");
    let detectedLanguage = $state("text");

    // Compute the actual language to apply to the editor
    let activeLanguage = $derived(
        language === "auto" ? detectedLanguage : language,
    );

    let showPreview = $state(true);

    const checkLanguage = debounce(async (content: string) => {
        if (language === "auto" && content) {
            const result = await languageDetector.detect(content);
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
</script>

<div class="flex flex-1 flex-col w-full relative min-h-0 min-w-0 h-full">
    <div class="flex flex-1 min-h-0 min-w-0 relative">
        <!-- Editor View -->
        <div
            class="flex-1 w-full min-w-0 {activeLanguage === 'markdown' &&
            showPreview
                ? 'border-r border-border md:w-1/2 flex-none'
                : ''}"
        >
            <Editor
                bind:value
                bind:line
                bind:col
                language={activeLanguage}
                bind:editorView
            />
        </div>

        <!-- Markdown Preview -->
        {#if activeLanguage === "markdown" && showPreview}
            <MarkdownPreview content={value} {editorView} />
        {/if}
    </div>
    <StatusBar
        {line}
        {col}
        bind:language
        {detectedLanguage}
        {activeLanguage}
        bind:showPreview
    />
</div>
