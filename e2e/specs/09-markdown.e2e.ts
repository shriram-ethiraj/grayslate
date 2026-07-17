import { browser, expect } from "@wdio/globals";
import {
  openExternalFixture,
  openExternalText,
  setMarkdownPreview,
} from "../helpers/app.js";

describe("Act 9 — Markdown mode", () => {
  it("renders headings, lists, code, and safe links through the Rust preview", async () => {
    await openExternalFixture("sample.md");
    const preview = await setMarkdownPreview(true);
    await browser.waitUntil(async () => (await preview.$("h1")).isExisting(), { timeout: 10_000 });

    expect(await (await preview.$("h1")).getText()).toBe("Grayslate sample");
    expect(await (await preview.$$("li")).length).toBe(2);
    expect(await (await preview.$("pre code")).getText()).toContain("const answer = 42");
    expect(await (await preview.$("a")).getAttribute("href")).toBe("https://example.com/");

    const families = await browser.execute(() => {
      const prose = document.querySelector<HTMLElement>("[data-testid='markdown-preview']");
      const code = prose?.querySelector<HTMLElement>("pre code");
      if (!prose || !code) throw new Error("Markdown typography elements are missing.");
      return {
        prose: getComputedStyle(prose).fontFamily,
        code: getComputedStyle(code).fontFamily,
      };
    });
    expect(families.prose).toContain("Source Sans 3");
    expect(families.prose).not.toContain("Commit Mono");
    expect(families.code).toContain("Commit Mono");
  });

  it("removes scripts, event handlers, and javascript URLs", async () => {
    const unsafe = "# Safe heading\n\n<script>window.__e2ePwned = true</script>\n" +
      "<img src=\"missing.png\" onerror=\"window.__e2ePwned = true\">\n" +
      "[unsafe](javascript:alert(1))";
    await openExternalText("unsafe.md", unsafe);
    const preview = await setMarkdownPreview(true);
    await browser.waitUntil(async () => (await preview.getText()).includes("Safe heading"), { timeout: 10_000 });

    expect(await (await preview.$$("script")).length).toBe(0);
    expect(await (await preview.$$("[onerror]")).length).toBe(0);
    expect(await (await preview.$$("a[href^='javascript:']")).length).toBe(0);
    expect(await browser.execute(() =>
      (window as unknown as { __e2ePwned?: boolean }).__e2ePwned ?? null,
    )).toBeNull();
  });

  it("synchronizes a long editor document into the preview scroll position", async () => {
    const longMarkdown = Array.from({ length: 100 }, (_, index) =>
      `## Section ${index + 1}\n\nParagraph ${index + 1} with enough content for scrolling.`,
    ).join("\n\n");
    await openExternalText("long.md", longMarkdown);
    const preview = await setMarkdownPreview(true);

    await browser.execute(() => {
      const scroller = document.querySelector<HTMLElement>(".cm-scroller");
      if (!scroller) throw new Error("Editor scroller missing.");
      scroller.dispatchEvent(new PointerEvent("pointerenter", { bubbles: true }));
      scroller.scrollTop = scroller.scrollHeight;
      scroller.dispatchEvent(new Event("scroll", { bubbles: false }));
    });
    await browser.waitUntil(async () => Number(await preview.getProperty("scrollTop")) > 0, {
      timeout: 10_000,
      interval: 200,
      timeoutMsg: "Markdown preview did not follow the editor scroll.",
    });
    await setMarkdownPreview(false);
  });
});
