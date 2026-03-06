<script lang="ts">
  import DOMPurify from "dompurify";
  import { Marked, type Token } from "marked";
  import { createScrollSync } from "./scrollSync";
  import type { EditorView } from "codemirror";
  import { onDestroy } from "svelte";

  let { content, editorView }: { content: string; editorView?: EditorView } =
    $props();

  let previewEl = $state<HTMLElement | undefined>(undefined);

  /** Block-level token types that advance the search offset in walkTokens. */
  const BLOCK_TOKENS = new Set([
    "heading",
    "paragraph",
    "code",
    "blockquote",
    "list",
    "table",
    "hr",
    "html",
  ]);

  /**
   * Build a line-offset lookup from source text.
   * lineStarts[i] = character offset where line (i+1) begins.
   */
  function buildLineStarts(src: string): number[] {
    const starts = [0];
    for (let i = 0; i < src.length; i++) {
      if (src[i] === "\n") starts.push(i + 1);
    }
    return starts;
  }

  function offsetToLine(lineStarts: number[], offset: number): number {
    let lo = 0,
      hi = lineStarts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (lineStarts[mid] <= offset) lo = mid;
      else hi = mid - 1;
    }
    return lo + 1;
  }

  /**
   * Renders markdown to HTML with data-line attributes on block-level elements.
   *
   * Uses marked's walkTokens hook to compute line numbers from token positions,
   * then custom renderer extensions inject data-line attributes.
   */
  function renderMarkdown(src: string): string {
    if (!src) return "";

    try {
      const lineStarts = buildLineStarts(src);
      const tokenLineMap = new WeakMap<Token, number>();
      let searchOffset = 0;

      const markedInstance = new Marked();
      markedInstance.use({
        walkTokens(token) {
          if (token.raw && typeof token.raw === "string") {
            const idx = src.indexOf(token.raw, searchOffset);
            if (idx !== -1) {
              tokenLineMap.set(token, offsetToLine(lineStarts, idx));
              if (BLOCK_TOKENS.has(token.type)) {
                searchOffset = idx + token.raw.length;
              }
            }
          }
        },
        renderer: {
          heading(token) {
            const line = tokenLineMap.get(token);
            const text = this.parser.parseInline(token.tokens);
            const attr = line != null ? ` data-line="${line}"` : "";
            return `<h${token.depth}${attr}>${text}</h${token.depth}>\n`;
          },
          paragraph(token) {
            const line = tokenLineMap.get(token);
            const text = this.parser.parseInline(token.tokens);
            const attr = line != null ? ` data-line="${line}"` : "";
            return `<p${attr}>${text}</p>\n`;
          },
          code(token) {
            const line = tokenLineMap.get(token);
            const attr = line != null ? ` data-line="${line}"` : "";
            const langClass = token.lang
              ? ` class="language-${token.lang}"`
              : "";
            const escaped = token.text
              .replace(/&/g, "&amp;")
              .replace(/</g, "&lt;")
              .replace(/>/g, "&gt;")
              .replace(/"/g, "&quot;");
            return `<pre${attr}><code${langClass}>${escaped}</code></pre>\n`;
          },
          blockquote(token) {
            const line = tokenLineMap.get(token);
            const body = this.parser.parse(token.tokens);
            const attr = line != null ? ` data-line="${line}"` : "";
            return `<blockquote${attr}>${body}</blockquote>\n`;
          },
          list(token) {
            const line = tokenLineMap.get(token);
            const tag = token.ordered ? "ol" : "ul";
            let body = "";

            for (const item of token.items) {
              const itemLine = tokenLineMap.get(item);
              const itemAttr =
                itemLine != null ? ` data-line="${itemLine}"` : "";
              let itemBody = this.parser.parse(item.tokens);

              if (item.task) {
                const checked = item.checked
                  ? ' checked="" disabled=""'
                  : ' disabled=""';
                itemBody = `<input type="checkbox"${checked}> ${itemBody}`;
              }

              body += `<li${itemAttr}>${itemBody}</li>\n`;
            }

            const attr = line != null ? ` data-line="${line}"` : "";
            const startAttr =
              token.ordered && token.start !== 1
                ? ` start="${token.start}"`
                : "";
            return `<${tag}${attr}${startAttr}>\n${body}</${tag}>\n`;
          },
          table(token) {
            const line = tokenLineMap.get(token);
            const attr = line != null ? ` data-line="${line}"` : "";

            let header = "<tr>";
            for (let i = 0; i < token.header.length; i++) {
              const cell = token.header[i];
              const align = token.align[i];
              const alignAttr = align ? ` style="text-align:${align}"` : "";
              const text = this.parser.parseInline(cell.tokens);
              header += `<th${alignAttr}>${text}</th>`;
            }
            header += "</tr>\n";

            let body = "";
            for (const row of token.rows) {
              body += "<tr>";
              for (let i = 0; i < row.length; i++) {
                const cell = row[i];
                const align = token.align[i];
                const alignAttr = align ? ` style="text-align:${align}"` : "";
                const text = this.parser.parseInline(cell.tokens);
                body += `<td${alignAttr}>${text}</td>`;
              }
              body += "</tr>\n";
            }

            return `<table${attr}>\n<thead>\n${header}</thead>\n<tbody>\n${body}</tbody>\n</table>\n`;
          },
          hr(token) {
            const line = tokenLineMap.get(token);
            const attr = line != null ? ` data-line="${line}"` : "";
            return `<hr${attr}>\n`;
          },
        },
      });

      const html = markedInstance.parse(src) as string;
      return DOMPurify.sanitize(html, {
        ADD_ATTR: ["data-line"],
      });
    } catch {
      return "<p>Error parsing markdown</p>";
    }
  }

  let htmlPreview = $derived(renderMarkdown(content));

  // Bidirectional scroll sync
  $effect(() => {
    if (!previewEl || !editorView) return;

    // Small delay so the preview DOM updates with new content first
    let syncCleanup: (() => void) | undefined;
    const timer = setTimeout(() => {
      syncCleanup = createScrollSync(editorView!, previewEl!);
    }, 50);

    return () => {
      clearTimeout(timer);
      syncCleanup?.();
    };
  });

  onDestroy(() => {
    // AGGRESSIVE MEMORY CLEANUP
    previewEl = undefined;
    content = "";
    htmlPreview = "";
  });
</script>

<div
  bind:this={previewEl}
  class="flex-1 w-full min-w-0 bg-background overflow-y-auto overscroll-none p-8 prose prose-sm dark:prose-invert max-w-none prose-pre:bg-[#ffffff] prose-pre:text-[#212121] dark:prose-pre:bg-[#23262E] dark:prose-pre:text-[#D5CED9] prose-code:text-[#212121] dark:prose-code:text-[#D5CED9] prose-pre:border prose-pre:border-border prose-pre:shadow-sm"
>
  {@html htmlPreview}
</div>
