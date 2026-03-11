<script lang="ts">
  import { tick } from "svelte";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import Input from "$lib/components/ui/input/input.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { editorGoToLine } from "$lib/editor/core/actions";
  import { editorState } from "$lib/state/editor.svelte.js";
  import type { EditorView } from "codemirror";

  let {
    open = $bindable(false),
    editorView,
    line = 1,
    lineCount = 1,
  }: {
    open: boolean;
    editorView: EditorView | undefined;
    line: number;
    lineCount: number;
  } = $props();

  let inputValue = $state("1");
  let errorMessage = $state("");
  let inputRef = $state<HTMLInputElement | null>(null);
  // Set to true in submit() so onCloseAutoFocus can focus the editor after the
  // dialog animation finishes, rather than returning focus to the trigger.
  let shouldFocusEditor = $state(false);

  const maxLine = $derived(Math.max(1, lineCount));

  const parsedLine = $derived.by(() => {
    const trimmed = inputValue.trim();
    if (!/^\d+$/.test(trimmed)) return undefined;
    return Number.parseInt(trimmed, 10);
  });

  const isValid = $derived.by(() => {
    const v = parsedLine;
    return v !== undefined && v >= 1 && v <= maxLine;
  });

  async function focusInput(): Promise<void> {
    await tick();
    inputRef?.focus();
    inputRef?.select();
  }

  // Pre-fill with the current line position whenever the dialog opens.
  $effect(() => {
    if (!open) return;
    inputValue = String(line);
    errorMessage = "";
    void focusInput();
  });

  // When Cmd/Ctrl+F is pressed while the dialog is open, close it and open
  // find-replace. The browser's native find bar is already blocked app-wide in
  // +layout.svelte, but the CodeMirror keymap won't fire here (editor has no
  // focus), so we handle it explicitly.
  $effect(() => {
    if (!open) return;
    function onWindowKeydown(event: KeyboardEvent): void {
      if ((event.metaKey || event.ctrlKey) && event.key === "f") {
        open = false;
        errorMessage = "";
        editorState.findReplace.visible = true;
        editorState.findReplace.replaceMode = false;
      }
    }
    window.addEventListener("keydown", onWindowKeydown, { capture: true });
    return () => window.removeEventListener("keydown", onWindowKeydown, { capture: true });
  });

  function handleInput(): void {
    if (errorMessage.length > 0) {
      errorMessage = "";
    }
  }

  function submit(): void {
    const v = parsedLine;
    if (v === undefined || v < 1 || v > maxLine) {
      errorMessage = `Enter a whole number from 1 to ${maxLine}.`;
      void focusInput();
      return;
    }

    if (!editorGoToLine(editorView, v, false)) {
      errorMessage = "Could not move the cursor in the editor.";
      return;
    }

    shouldFocusEditor = true;
    open = false;
    errorMessage = "";
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content
    class="sm:max-w-[25rem]"
    onOpenAutoFocus={(event) => {
      event.preventDefault();
      void focusInput();
    }}
    onCloseAutoFocus={(event) => {
      if (shouldFocusEditor) {
        event.preventDefault();
        shouldFocusEditor = false;
        editorView?.focus();
      }
    }}
  >
    <form
      class="grid gap-3"
      onsubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <div class="grid gap-2">
        <label class="text-sm font-medium text-foreground" for="go-to-line-input">
          Go to line (1–{maxLine})
        </label>
        <Input
          id="go-to-line-input"
          bind:ref={inputRef}
          bind:value={inputValue}
          type="text"
          inputmode="numeric"
          autocomplete="off"
          spellcheck="false"
          aria-invalid={errorMessage.length > 0 || !isValid}
          aria-describedby="go-to-line-error"
          placeholder={`1–${maxLine}`}
          oninput={handleInput}
        />
        {#if errorMessage.length > 0}
          <p id="go-to-line-error" class="text-xs text-destructive">
            {errorMessage}
          </p>
        {/if}
      </div>

      <Dialog.Footer>
        <Button type="submit">Go</Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
