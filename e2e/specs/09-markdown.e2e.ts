import { $, $$, browser, expect } from "@wdio/globals";
import { scenario } from "../coverage/scenario.js";
import {
  queueExternalConfirmation,
  waitForExternalAction,
} from "../driver/externalAction.js";
import { END, MOD } from "../driver/keys.js";
import {
  armOperationGate,
  releaseOperationGate,
  waitForOperationGate,
} from "../driver/operationGate.js";
import {
  readDocumentSelectionText,
  readWindowGlobal,
} from "../driver/probe.js";
import { waitFor } from "../driver/wait.js";
import {
  openFixture,
  openText,
  provisionBytes,
} from "../fixtures/factories.js";
import { waitForClipboardText } from "../driver/clipboard.js";
import * as editor from "../pages/editor.js";
import * as markdown from "../pages/markdown.js";

/**
 * Markdown preview.
 *
 * Rendering and sanitization happen in Rust (`pulldown-cmark` + `ammonia`), so
 * the preview's DOM *is* the sanitizer's output. The sanitization scenario
 * asserts the payload's side effect never fired, not merely that the markup was
 * stripped — a sanitizer that leaves an inert-looking tag behind but still
 * executes would pass a markup-only check.
 */
describe("Markdown preview", () => {
  scenario(
    "markdown.preview.toggle",
    "opens and closes preview without losing the editor session or history",
    async () => {
      await openFixture("sample.md", "toggle.md");
      const original = await editor.text();
      await editor.focus();
      await browser.keys([MOD, END]);
      await editor.type("\ncontinuity edit");
      await editor.waitForText(
        (text) => text.endsWith("continuity edit"),
        "The pre-preview edit was not applied.",
      );

      await markdown.setVisible(true);
      expect(await (await markdown.pane()).isDisplayed()).toBe(true);

      await markdown.setVisible(false);
      expect(await (await markdown.pane()).isExisting()).toBe(false);

      await editor.focus();
      await editor.undo();
      await editor.waitForExactText(original);
    },
  );

  scenario(
    "markdown.preview.renders",
    "renders headings, lists, code, and links",
    async () => {
      await openFixture("sample.md", "renders.md");
      await markdown.setVisible(true);

      await waitFor(async () => (await markdown.within("h1")).isExisting(), {
        message: "The preview never rendered a heading.",
      });
      expect(await (await markdown.within("h1")).getText()).toBe("Grayslate sample");

      const items = await $$("[data-testid='markdown-preview'] li");
      expect(items.length).toBe(2);

      const code = await markdown.within("pre");
      expect(await code.getText()).toContain("const answer = 42");

      // The href must survive sanitization intact, not be rewritten or dropped.
      const link = await markdown.within("a");
      expect(await link.getAttribute("href")).toBe("https://example.com/");
    },
  );

  scenario(
    "markdown.sanitization",
    "strips scripts, event handlers, and javascript: URLs without executing them",
    async () => {
      await openText(
        "unsafe.md",
        [
          "# Unsafe",
          "",
          "<script>window.__e2ePwned = 'script';</script>",
          "",
          "<img src=x onerror=\"window.__e2ePwned = 'onerror'\">",
          "",
          "[click](javascript:window.__e2ePwned='href')",
          "",
        ].join("\n"),
      );
      await markdown.setVisible(true);
      await markdown.waitForRendered();

      expect(await (await $$("[data-testid='markdown-preview'] script")).length).toBe(0);
      expect(await (await $$("[data-testid='markdown-preview'] [onerror]")).length).toBe(0);
      expect(
        await (await $$("[data-testid='markdown-preview'] a[href^='javascript:']")).length,
      ).toBe(0);

      // The load-bearing assertion: nothing ran. Stripped markup alone would
      // not prove that.
      expect(await readWindowGlobal("__e2ePwned")).toBeNull();
    },
  );

  scenario(
    "markdown.scroll-sync",
    "moves the preview to the position corresponding to the editor",
    async () => {
      // Long enough that both panes scroll well past a single viewport.
      const body = Array.from({ length: 300 }, (_, i) => `## Heading ${i}\n\nBody ${i}\n`).join(
        "\n",
      );
      await openText("scroll-sync.md", body);
      await markdown.setVisible(true);
      await markdown.waitForRendered();

      const height = await markdown.scrollHeight();
      const visible = await markdown.clientHeight();
      expect(height).toBeGreaterThan(visible);

      // Move the caret to the end of the document with the keyboard. That is a
      // real user action that scrolls the editor, unlike `scrollIntoView`,
      // which CodeMirror's virtualized scroller ignores.
      await editor.focus();
      await browser.keys([MOD, END]);

      await markdown.waitForScroll(
        (top) => top > visible,
        "The preview did not follow the editor past a single viewport.",
      );
    },
  );

  scenario(
    "markdown.copy",
    "selects all and copies from the preview context menu",
    async () => {
      await openFixture("sample.md", "copy.md");
      await markdown.setVisible(true);
      await markdown.waitForRendered();

      await markdown.openContextMenu();
      await markdown.chooseContextMenuItem("select-all");
      const selected = await readDocumentSelectionText();
      expect(selected).toContain("Grayslate sample");
      expect(selected).toContain("const answer = 42");

      await markdown.openContextMenu();
      await markdown.chooseContextMenuItem("copy");
      await waitForClipboardText(selected);
      await markdown.setVisible(false);
    },
  );

  scenario(
    "markdown.external-links",
    "validates and hands external links to the OS without navigating the webview",
    async () => {
      await openText(
        "external-link.md",
        "[Open documentation](https://example.com/docs?from=grayslate)\n",
      );
      await markdown.setVisible(true);
      await markdown.waitForRendered();
      await queueExternalConfirmation(true);

      const webviewUrl = await browser.getUrl();
      await (await markdown.within("a")).click();
      expect(await waitForExternalAction()).toEqual({
        kind: "open-url",
        target: "https://example.com/docs?from=grayslate",
      });
      expect(await browser.getUrl()).toBe(webviewUrl);
    },
  );

  scenario(
    "markdown.relative-images",
    "loads a saved document's relative image through the bounded asset command",
    async () => {
      // A valid 1×1 transparent PNG next to the Markdown file.
      provisionBytes(
        "pixel.png",
        Buffer.from(
          "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
          "base64",
        ),
      );
      await openText("relative-image.md", "![one pixel](pixel.png)\n");
      await markdown.setVisible(true);

      const image = await markdown.within("img");
      await waitFor(
        async () => (await image.getAttribute("src"))?.startsWith("blob:") === true,
        { message: "The relative Markdown image never resolved to a bounded blob URL." },
      );
      expect(await image.getAttribute("alt")).toBe("one pixel");
    },
  );

  scenario(
    "markdown.size-guard",
    "refuses preview rendering above the five-megabyte limit",
    async () => {
      const oversized = `# Oversized\n${"x".repeat(5 * 1024 * 1024)}`;
      await openText("oversized.md", oversized);
      await markdown.setVisible(true);

      await waitFor(
        async () =>
          (await markdown.pane()).getText().then((text) => text.includes("up to 5 MB")),
        { message: "Oversized Markdown did not show the preview limit notice." },
      );
      expect(await (await markdown.within("h1")).isExisting()).toBe(false);
      await markdown.setVisible(false);
      await editor.focus();
      await editor.type("x");
      await editor.waitForLength(oversized.length + 1);
    },
  );

  scenario(
    "markdown.cancel",
    "cancels an in-flight render when preview is closed",
    async () => {
      const source = `${"# Heading\n\nbody text\n\n".repeat(20_000)}`;
      await markdown.setVisible(false);
      await openText("markdown-cancel.md", source);
      await armOperationGate("markdown-render");
      await markdown.requestVisible(true);
      await waitForOperationGate("markdown-render");

      try {
        await markdown.requestVisible(false);
      } finally {
        await releaseOperationGate("markdown-render");
      }

      expect(await (await markdown.pane()).isExisting()).toBe(false);
      await editor.focus();
      await editor.type("x");
      await editor.waitForLength(source.length + 1);
    },
  );
});
