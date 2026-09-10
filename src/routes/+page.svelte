<script lang="ts">
  import { onMount, type Component } from "svelte";
  import StartupReveal from "$lib/components/StartupReveal.svelte";
  import { beginTrackedWork } from "virtual:grayslate-e2e-runtime";

  type EditorWrapperProps = { onReady?: () => void };

  let EditorWrapper = $state<Component<EditorWrapperProps> | undefined>(undefined);
  let editorReady = $state(false);
  let loadError = $state<string | undefined>(undefined);
  const revealReady = $derived(editorReady || loadError !== undefined);

  onMount(() => {
    let cancelled = false;
    const finishTracking = beginTrackedWork("lazy-editor-import");
    performance.mark("grayslate:editor-import-start");

    void import("$lib/editor/components/EditorWrapper.svelte")
      .then((module) => {
        if (cancelled) return;
        EditorWrapper = module.default;
        performance.mark("grayslate:editor-import-ready");
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        loadError = error instanceof Error ? error.message : String(error);
      })
      .finally(finishTracking);

    return () => {
      cancelled = true;
    };
  });
</script>

<main class="relative flex min-h-0 flex-1" data-testid="editor-bootstrap">
  <StartupReveal ready={revealReady} variant="editor">
    {#if EditorWrapper}
      <EditorWrapper onReady={() => { editorReady = true; }} />
    {:else if loadError}
      <div class="flex min-h-0 flex-1 items-center justify-center p-6 text-sm text-destructive" role="alert">
        Grayslate could not load the editor: {loadError}
      </div>
    {/if}
  </StartupReveal>
</main>
